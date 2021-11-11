use assert_cmd::Command;
use std::ffi::OsStr;
use std::fmt::Display;

#[macro_export]
macro_rules! declare_tests {
    (@internal missing_key(file) $prefix:expr, $name:ident) => {
        concat!($prefix, stringify!($name), ".pl")
    };
    (@internal missing_key($key:tt) $prefix:expr, $name:ident) => {
        ""
    };
    (@internal missing_key($key:tt) $prefix:expr, $name:ident, $value:expr) => {
        $value
    };

    (@internal missing_value(in) $prefix:expr, $name:ident) => {
        include_str!(concat!("../../", $prefix, stringify!($name), ".stdin"))
    };
    (@internal missing_value(err) $prefix:expr, $name:ident) => {
        include_str!(concat!("../../", $prefix, stringify!($name), ".stderr"))
    };
    (@internal missing_value(out) $prefix:expr, $name:ident) => {
        include_str!(concat!("../../", $prefix, stringify!($name), ".stdout"))
    };
    (@internal missing_value($key:tt) $prefix:expr, $name:ident, $value:expr) => {
        $value
    };

    (@internal test(toplevel) $prefix:expr => $(#[$m:meta])* $name:ident => file $file:expr => in $in:expr => out $out:expr => err $err:expr) => {
        $(#[$m])*
        #[test]
        fn $name() {
            $crate::helper::run_top_level_test_no_args($in, $out, $err);
        }
    };
    (@internal test(toplevel) $prefix:expr => $(#[$m:meta])* $name:ident => file $file:expr => in $in:expr => out $out:expr => err $err:expr => with $args:expr) => {
        $(#[$m])*
        #[test]
        fn $name() {
            $crate::helper::run_top_level_test_with_args($args, $in, $out, $err);
        }
    };
    (@internal test(load_module) $prefix:expr => $(#[$m:meta])* $name:ident => file $file:expr => in $in:expr => out $out:expr => err $err:expr $(=> with $args:expr)?) => {
        $(#[$m])*
        #[test]
        fn $name() {
            $crate::helper::load_module_test($file, $out, $err);
        }
    };

    (@internal fill_keys $prefix:expr => $(#[$m:meta])* $kind:ident $name:ident $(=> file $file:expr)? $(=> in $in:expr)? $(=> out $out:expr)? $(=> err $err:expr)? $(=> with $args:expr)?) => {
            // insert missing keys
            $crate::declare_tests!(
                @internal test($kind) $prefix => $(#[$m])* $name
                    => file $crate::declare_tests!(@internal missing_key(file) $prefix, $name $(, $file)?)
                    => in   $crate::declare_tests!(@internal missing_key(in  ) $prefix, $name $(, $in  )?)
                    => out  $crate::declare_tests!(@internal missing_key(out ) $prefix, $name $(, $out )?)
                    => err  $crate::declare_tests!(@internal missing_key(err ) $prefix, $name $(, $err )?)
                    $(=> with $args)?
            );
    };

    ($prefix:expr => $($(#[$m:meta])* $kind:ident $name:ident $(=> $key:tt $($value:expr)?)*),+$(,)?) => {
        $(
            // insert missing values for existing keys
            $crate::declare_tests!(@internal fill_keys $prefix => $(#[$m])* $kind $name
                $(=> $key $crate::declare_tests!(@internal missing_value($key) $prefix, $name $(, $value)?))*
            );
        )+
    };
}

pub(crate) trait Expectable {
    #[track_caller]
    fn assert_eq(self, other: &[u8], args: impl Display);
}

impl Expectable for &str {
    #[track_caller]
    fn assert_eq(self, other: &[u8], args: impl Display) {
        if let Ok(other_str) = std::str::from_utf8(other) {
            assert_eq!(other_str, self, "{}", args)
        } else {
            // should always fail as other is not valid utf-8 but self is
            // just for consistent assert error message
            assert_eq!(other, self.as_bytes())
        }
    }
}

impl Expectable for &[u8] {
    #[track_caller]
    fn assert_eq(self, other: &[u8], args: impl Display) {
        assert_eq!(other, self, "{}", args)
    }
}

/// Tests whether the file can be successfully loaded
/// and produces the expected output during it
#[track_caller]
pub(crate) fn load_module_test<T: Expectable>(file: &str, expected_out: T, expected_err: T) {
    use scryer_prolog::*;

    let input = machine::Stream::from("");
    let output = machine::Stream::from(String::new());
    let error = machine::Stream::from(String::new());

    let mut wam = machine::Machine::new(input, output.clone(), error.clone());

    wam.load_file(
        file.into(),
        machine::Stream::from(
            std::fs::read_to_string(AsRef::<std::path::Path>::as_ref(file))
                .map_err(|err| (err, file))
                .unwrap(),
        ),
    );

    let output = output.bytes().unwrap();
    let error = error.bytes().unwrap();
    expected_out.assert_eq(output.as_slice(), "Stdout");
    expected_err.assert_eq(error.as_slice(), "Stderr");
}

pub const SCRYER_PROLOG: &str = "scryer-prolog";

#[track_caller]
pub fn run_top_level_test_no_args<
    S: Into<Vec<u8>>,
    O: assert_cmd::assert::IntoOutputPredicate<OP>,
    E: assert_cmd::assert::IntoOutputPredicate<EP>,
    OP: predicates_core::Predicate<[u8]>,
    EP: predicates_core::Predicate<[u8]>,
>(
    stdin: S,
    expected_stdout: O,
    expected_stderr: E,
) {
    run_top_level_test_with_args::<&[&str], _, _, _, _, _, _>(
        &[],
        stdin,
        expected_stdout,
        expected_stderr,
    )
}

/// Test whether scryer-prolog
/// produces the expected output when called with the supplied
/// arguments and fed the supplied input
#[track_caller]
pub fn run_top_level_test_with_args<
    A: IntoIterator<Item = AS>,
    S: Into<Vec<u8>>,
    O: assert_cmd::assert::IntoOutputPredicate<OP>,
    E: assert_cmd::assert::IntoOutputPredicate<EP>,
    AS: AsRef<OsStr>,
    OP: predicates_core::Predicate<[u8]>,
    EP: predicates_core::Predicate<[u8]>,
>(
    args: A,
    stdin: S,
    expected_stdout: O,
    expected_stderr: E,
) {
    Command::cargo_bin(SCRYER_PROLOG)
        .unwrap()
        .arg("-f")
        .args(args)
        .write_stdin(stdin)
        .assert()
        .stdout(expected_stdout.into_output())
        .stderr(expected_stderr.into_output())
        .success();
}
