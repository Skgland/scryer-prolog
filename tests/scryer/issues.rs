crate::declare_tests!(
  "tests-pl/issue/" =>
  toplevel issue807_ignored_constraint => in => out,
  // FIXME: the line number is of by one (should be 4), empty line not accounted for or starting to count at line 0?
  toplevel issue812_singleton_warning => in => out,
  toplevel issue815_no_stutter => in => out,
  toplevel issue820_multiple_goals
        => out "helloworld\n"
        => with &["-g", "test", "-g", "halt", "tests-pl/issue/issue820_goals.pl"],
  toplevel issue820_compound_goal
        => out "helloworld\n"
        => with &["-g", "test,halt"         , "tests-pl/issue/issue820_goals.pl"],
  load_module issue831_call0 => in => out,
  toplevel issue839_run_top_level_test_with_args
        => with &["tests-pl/issue/issue839_op3.pl"],
  toplevel issue841_occurs_check_flag => in => out
        => with &["tests-pl/issue/issue841_occurs_check_flag.pl"],
  toplevel issue841_occurs_check_flag2 => in => out,
  toplevel issue844_handle_residual_goal => in => out,
  toplevel issue852_do_not_duplicate_path_components => in => out,
  toplevel issue857_display_constraints => in => out,
);
