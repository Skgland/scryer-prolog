%% for builtins that are not part of the ISO standard.
%% must be loaded at the REPL with

%% ?- use_module(library(iso_ext)).

:- module(iso_ext, [bb_b_put/2, bb_get/2, bb_put/2, call_cleanup/2,
		    call_with_inference_limit/3, forall/2, maybe/0,
		    partial_string/1, partial_string/3,
		    partial_string_tail/2, set_random/1,
		    setup_call_cleanup/3, variant/2]).

forall(Generate, Test) :-
    \+ (Generate, \+ Test).

%% (non-)backtrackable global variables.

bb_put(Key, Value) :- atom(Key), !, '$store_global_var'(Key, Value).
bb_put(Key, _) :- throw(error(type_error(atom, Key), bb_put/2)).

%% backtrackable global variables.

bb_b_put(Key, NewValue) :-
    (  '$bb_get_with_offset'(Key, OldValue, OldOffset) ->
       call_cleanup((store_global_var_with_offset(Key, NewValue) ; false),
		    reset_global_var_at_offset(Key, OldValue, OldOffset))
    ;  call_cleanup((store_global_var_with_offset(Key, NewValue) ; false),
		    reset_global_var_at_key(Key))
    ).

store_global_var_with_offset(Key, Value) :- '$store_global_var_with_offset'(Key, Value).

store_global_var(Key, Value) :- '$store_global_var'(Key, Value).

reset_global_var_at_key(Key) :- '$reset_global_var_at_key'(Key).

reset_global_var_at_offset(Key, Value, Offset) :- '$reset_global_var_at_offset'(Key, Value, Offset).

'$bb_get_with_offset'(Key, OldValue, Offset) :-
    atom(Key), !, '$fetch_global_var_with_offset'(Key, OldValue, Offset).
'$bb_get_with_offset'(Key, _, _) :-
    throw(error(type_error(atom, Key), bb_b_put/2)).

bb_get(Key, Value) :- atom(Key), !, '$fetch_global_var'(Key, Value).
bb_get(Key, _) :- throw(error(type_error(atom, Key), bb_get/2)).

call_cleanup(G, C) :- setup_call_cleanup(true, G, C).


% setup_call_cleanup.

setup_call_cleanup(S, G, C) :-
    '$get_b_value'(B),
    call(S),
    '$set_cp_by_default'(B),
    '$get_current_block'(Bb),
    (  '$call_with_default_policy'(var(C)) ->
       throw(error(instantiation_error, setup_call_cleanup/3))
    ;  '$call_with_default_policy'(scc_helper(C, G, Bb))
    ).

:- non_counted_backtracking scc_helper/3.
scc_helper(C, G, Bb) :-
    '$get_cp'(Cp), '$install_scc_cleaner'(C, NBb), call(G),
    ( '$check_cp'(Cp) ->
      '$reset_block'(Bb),
      '$call_with_default_policy'(run_cleaners_without_handling(Cp))
    ; '$call_with_default_policy'(true)
    ; '$reset_block'(NBb),
      '$fail').
scc_helper(_, _, Bb) :-
    '$reset_block'(Bb), '$get_ball'(Ball),
    '$call_with_default_policy'(run_cleaners_with_handling),
    '$erase_ball',
    '$call_with_default_policy'(throw(Ball)).
scc_helper(_, _, _) :-
    '$get_cp'(Cp),
    '$call_with_default_policy'(run_cleaners_without_handling(Cp)),
    '$fail'.

:- non_counted_backtracking run_cleaners_with_handling/0.
run_cleaners_with_handling :-
    '$get_scc_cleaner'(C), '$get_level'(B),
    '$call_with_default_policy'(catch(C, _, true)),
    '$set_cp_by_default'(B),
    '$call_with_default_policy'(run_cleaners_with_handling).
run_cleaners_with_handling :-
    '$restore_cut_policy'.

:- non_counted_backtracking run_cleaners_without_handling/1.
run_cleaners_without_handling(Cp) :-
    '$get_scc_cleaner'(C),
    '$get_level'(B),
    call(C),
    '$set_cp_by_default'(B),
    '$call_with_default_policy'(run_cleaners_without_handling(Cp)).
run_cleaners_without_handling(Cp) :-
    '$set_cp_by_default'(Cp),
    '$restore_cut_policy'.

% call_with_inference_limit

:- non_counted_backtracking end_block/4.
end_block(_, Bb, NBb, L) :-
    '$clean_up_block'(NBb),
    '$reset_block'(Bb).
end_block(B, Bb, NBb, L) :-
    '$install_inference_counter'(B, L, _),
    '$reset_block'(NBb),
    '$fail'.

:- non_counted_backtracking handle_ile/3.
handle_ile(B, inference_limit_exceeded(B), inference_limit_exceeded) :- !.
handle_ile(B, E, _) :-
    '$remove_call_policy_check'(B),
    '$call_with_default_policy'(throw(E)).

call_with_inference_limit(G, L, R) :-
    '$get_current_block'(Bb),
    '$get_b_value'(B),
    '$call_with_default_policy'(call_with_inference_limit(G, L, R, Bb, B)),
    '$remove_call_policy_check'(B).

:- non_counted_backtracking call_with_inference_limit/5.
call_with_inference_limit(G, L, R, Bb, B) :-
    '$install_new_block'(NBb),
    '$install_inference_counter'(B, L, Count0),
    call(G),
    '$inference_level'(R, B),
    '$remove_inference_counter'(B, Count1),
    '$call_with_default_policy'(is(Diff, L - (Count1 - Count0))),
    '$call_with_default_policy'(end_block(B, Bb, NBb, Diff)).
call_with_inference_limit(_, _, R, Bb, B) :-
    '$reset_block'(Bb),
    '$remove_inference_counter'(B, _),
    (  '$get_ball'(Ball),
       '$get_level'(Cp),
       '$set_cp_by_default'(Cp)
    ;  '$remove_call_policy_check'(B),
       '$fail'
    ),
    '$erase_ball',
    '$call_with_default_policy'(handle_ile(B, Ball, R)).

variant(X, Y) :- '$variant'(X, Y).

% succeeds with probability 0.5.
maybe :- '$maybe'.

set_random(Seed) :-
    (  nonvar(Seed) ->
       (  Seed = seed(S) ->
	  (  var(S) -> throw(error(instantiation_error, set_random/1))
	  ;  integer(S) -> '$set_seed'(S)
	  ;  throw(error(type_error(integer(S), set_random/1)))
	  )
       )
    ;  throw(error(instantiation_error, set_random/1))
    ).

partial_string(String, L, L0) :-
    (  String == [] ->
       L = L0
    ;  catch(atom_chars(Atom, String),
	     error(E, _),
	     throw(error(E, partial_string/3))),
       '$create_partial_string'(Atom, L, L0)
    ).

partial_string(String) :-
    '$is_partial_string'(String).

partial_string_tail(String, Tail) :-
    (  partial_string(String) ->
       '$partial_string_tail'(String, Tail)
    ;  throw(error(type_error(partial_string, String), partial_string_tail/2))
    ).
