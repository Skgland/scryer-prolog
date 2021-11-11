crate::declare_tests!(
    "tests-pl/src_test/" =>
    load_module syntax_error
        => out "caught: error(syntax_error(incomplete_reduction),read_term/3:6)\n"
);

crate::declare_tests!(
    "src/tests/" =>
    load_module builtins,
    load_module call_with_inference_limit,
    load_module facts,
    load_module hello_world
        => out "Hello World!\n",
    load_module rules,

    #[ignore] // fails to halt
    load_module predicates,

    #[ignore] // var ids sometimes differ
    load_module setup_call_cleanup
        => out "1+21+31+2>_13169+_131701+_121851+2>41+2>_131701+2>31+2>31+2>4ba",

    #[ignore] // var ids sometimes differ
    toplevel setup_call_cleanup_process
        => out "1+21+31+2>_14107+_141081+_131231+2>41+2>_141081+2>31+2>31+2>4ba"
        => with &["src/tests/setup_call_cleanup.pl"]
);
crate::declare_tests!(
    "src/tests/clpz/" =>
    load_module test_clpz
);
