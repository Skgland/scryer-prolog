use prolog_parser::ast::*;
use prolog_parser::tabled_rc::*;

use prolog::forms::*;
use prolog::machine::code_repo::*;
use prolog::machine::machine_errors::*;
use prolog::machine::machine_indices::*;

use std::collections::VecDeque;

// Module's and related types are defined in forms.
impl Module {
    pub fn new(module_decl: ModuleDecl, atom_tbl: TabledData<Atom>) -> Self
    {
        Module {
            module_decl,
            atom_tbl,
            user_term_expansions: (Predicate::new(), VecDeque::from(vec![])),
            user_goal_expansions: (Predicate::new(), VecDeque::from(vec![])),
            term_expansions: (Predicate::new(), VecDeque::from(vec![])),
            goal_expansions: (Predicate::new(), VecDeque::from(vec![])),
            code_dir: CodeDir::new(),
            op_dir: default_op_dir(),
            inserted_expansions: false,
        }
    }

    pub fn dump_expansions(
        &self,
        code_repo: &mut CodeRepo,
        flags: MachineFlags,
    ) -> Result<(), ParserError> {
        {
            let te = code_repo
                .term_dir
                .entry((clause_name!("term_expansion"), 2))
                .or_insert((Predicate::new(), VecDeque::from(vec![])));

            (te.0)
                .0
                .extend((self.user_term_expansions.0).0.iter().cloned());
            te.1.extend(self.user_term_expansions.1.iter().cloned());
        }

        {
            let ge = code_repo
                .term_dir
                .entry((clause_name!("goal_expansion"), 2))
                .or_insert((Predicate::new(), VecDeque::from(vec![])));

            (ge.0)
                .0
                .extend((self.user_goal_expansions.0).0.iter().cloned());
            ge.1.extend(self.user_goal_expansions.1.iter().cloned());
        }

        code_repo.compile_hook(CompileTimeHook::TermExpansion, flags)?;
        code_repo.compile_hook(CompileTimeHook::GoalExpansion, flags)?;

        Ok(())
    }

    pub fn add_module_expansion_record(
        &mut self,
        hook: CompileTimeHook,
        clause: PredicateClause,
        queue: VecDeque<TopLevel>,
    ) {
        match hook {
            CompileTimeHook::TermExpansion | CompileTimeHook::UserTermExpansion => {
                (self.term_expansions.0).0.push(clause);
                self.term_expansions.1.extend(queue.into_iter());
            }
            CompileTimeHook::GoalExpansion | CompileTimeHook::UserGoalExpansion => {
                (self.goal_expansions.0).0.push(clause);
                self.goal_expansions.1.extend(queue.into_iter());
            }
        }
    }
}

pub trait SubModuleUser {
    fn atom_tbl(&self) -> TabledData<Atom>;
    fn op_dir(&mut self) -> &mut OpDir;
    fn remove_code_index(&mut self, PredicateKey);
    fn get_code_index(&self, PredicateKey, ClauseName) -> Option<CodeIndex>;

    fn insert_dir_entry(&mut self, ClauseName, usize, CodeIndex);

    fn get_op_module_name(&mut self, name: ClauseName, fixity: Fixity) -> Option<ClauseName> {
        self.op_dir()
            .get(&(name, fixity))
            .map(|op_val| op_val.owning_module())
    }

    fn remove_module(&mut self, mod_name: ClauseName, module: &Module) {
        for (name, arity) in module.module_decl.exports.iter().cloned() {
            let name = name.defrock_brackets();

            match self.get_code_index((name.clone(), arity), mod_name.clone()) {
                Some(CodeIndex(ref code_idx)) => {
                    if &code_idx.borrow().1 != &module.module_decl.name {
                        continue;
                    }

                    self.remove_code_index((name.clone(), arity));

                    // remove or respecify ops.
                    if arity == 2 {
                        if let Some(mod_name) = self.get_op_module_name(name.clone(), Fixity::In) {
                            if mod_name == module.module_decl.name {
                                self.op_dir().remove(&(name.clone(), Fixity::In));
                            }
                        }
                    } else if arity == 1 {
                        if let Some(mod_name) = self.get_op_module_name(name.clone(), Fixity::Pre) {
                            if mod_name == module.module_decl.name {
                                self.op_dir().remove(&(name.clone(), Fixity::Pre));
                            }
                        }

                        if let Some(mod_name) = self.get_op_module_name(name.clone(), Fixity::Post)
                        {
                            if mod_name == module.module_decl.name {
                                self.op_dir().remove(&(name.clone(), Fixity::Post));
                            }
                        }
                    }
                }
                _ => {}
            };
        }
    }

    // returns true on successful import.
    fn import_decl(&mut self, name: ClauseName, arity: usize, submodule: &Module) -> bool {
        let name = name.defrock_brackets();
        let mut found_op = false;

        {
            let mut insert_op_dir = |fix| {
                if let Some(op_data) = submodule.op_dir.get(&(name.clone(), fix)) {
                    self.op_dir().insert((name.clone(), fix), op_data.clone());
                    found_op = true;
                }
            };

            if arity == 1 {
                insert_op_dir(Fixity::Pre);
                insert_op_dir(Fixity::Post);
            } else if arity == 2 {
                insert_op_dir(Fixity::In);
            }
        }

        if let Some(code_data) = submodule.code_dir.get(&(name.clone(), arity)) {
            let name = name.with_table(submodule.atom_tbl.clone());
            let atom_tbl = self.atom_tbl();

            atom_tbl.borrow_mut().insert(name.to_rc());

            self.insert_dir_entry(name, arity, code_data.clone());
            true
        } else {
            found_op
        }
    }

    fn use_qualified_module(
        &mut self,
        &mut CodeRepo,
        MachineFlags,
        &Module,
        &Vec<PredicateKey>,
    ) -> Result<(), SessionError>;
    fn use_module(&mut self, &mut CodeRepo, MachineFlags, &Module) -> Result<(), SessionError>;
}

pub fn use_qualified_module<User>(
    user: &mut User,
    submodule: &Module,
    exports: &Vec<PredicateKey>,
) -> Result<(), SessionError>
where
    User: SubModuleUser,
{
    for (name, arity) in exports.iter().cloned() {
        if !submodule
            .module_decl
            .exports
            .contains(&(name.clone(), arity))
        {
            continue;
        }

        if !user.import_decl(name, arity, submodule) {
            return Err(SessionError::ModuleDoesNotContainExport);
        }
    }

    Ok(())
}

pub fn use_module<User: SubModuleUser>(
    user: &mut User,
    submodule: &Module,
) -> Result<(), SessionError> {
    for (name, arity) in submodule.module_decl.exports.iter().cloned() {
        if !user.import_decl(name, arity, submodule) {
            return Err(SessionError::ModuleDoesNotContainExport);
        }
    }

    Ok(())
}

impl SubModuleUser for Module {
    fn atom_tbl(&self) -> TabledData<Atom> {
        self.atom_tbl.clone()
    }

    fn op_dir(&mut self) -> &mut OpDir {
        &mut self.op_dir
    }

    fn get_code_index(&self, key: PredicateKey, _: ClauseName) -> Option<CodeIndex> {
        self.code_dir.get(&key).cloned()
    }

    fn remove_code_index(&mut self, key: PredicateKey) {
        self.code_dir.remove(&key);
    }

    fn insert_dir_entry(&mut self, name: ClauseName, arity: usize, idx: CodeIndex) {
        self.code_dir.insert((name, arity), idx);
    }

    fn use_qualified_module(
        &mut self,
        _: &mut CodeRepo,
        _: MachineFlags,
        submodule: &Module,
        exports: &Vec<PredicateKey>,
    ) -> Result<(), SessionError> {
        use_qualified_module(self, submodule, exports)?;

        (self.user_term_expansions.0)
            .0
            .extend((submodule.term_expansions.0).0.iter().cloned());
        self.user_term_expansions
            .1
            .extend(submodule.term_expansions.1.iter().cloned());

        (self.user_goal_expansions.0)
            .0
            .extend((submodule.goal_expansions.0).0.iter().cloned());
        self.user_goal_expansions
            .1
            .extend(submodule.goal_expansions.1.iter().cloned());

        Ok(())
    }

    fn use_module(
        &mut self,
        _: &mut CodeRepo,
        _: MachineFlags,
        submodule: &Module,
    ) -> Result<(), SessionError> {
        use_module(self, submodule)?;

        (self.user_term_expansions.0)
            .0
            .extend((submodule.term_expansions.0).0.iter().cloned());
        self.user_term_expansions
            .1
            .extend(submodule.term_expansions.1.iter().cloned());

        (self.user_goal_expansions.0)
            .0
            .extend((submodule.goal_expansions.0).0.iter().cloned());
        self.user_goal_expansions
            .1
            .extend(submodule.goal_expansions.1.iter().cloned());
	
        Ok(())
    }
}
