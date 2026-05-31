use core::ffi::c_int;
#[cfg(not(windows))]
use core::ffi::c_void;
use std::io::ErrorKind;
use std::sync::atomic::{self, AtomicU32};
use std::sync::{LazyLock, Mutex};

#[cfg(not(windows))]
use libc::sigaction;

use crate::atom_table::Atom;
use crate::parser::ast::{Fixnum, Literal};

#[derive(Debug, Clone, Copy)]
pub enum SignalHandler {
    #[cfg(windows)]
    Address(usize),
    #[cfg(not(windows))]
    Handler(unsafe extern "C" fn(signal: c_int)),
    #[cfg(not(windows))]
    SigAction(unsafe extern "C" fn(signal: c_int, *const libc::siginfo_t, *const c_void)),
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum SignalAction {
    Default,
    Ignore,
    Handler(SignalHandler),
    DoNothing,
    Machine(MachineAction),
}

impl SignalAction {
    #[cfg(not(windows))]
    fn to_action(&self) -> sigaction {
        use std::mem::MaybeUninit;

        let mut act = unsafe { MaybeUninit::<sigaction>::zeroed().assume_init() };

        act.sa_sigaction = self.to_handler_address();

        if let SignalAction::Handler(SignalHandler::SigAction(_)) = self {
            act.sa_flags |= libc::SA_SIGINFO;
        }

        act
    }

    fn to_handler_address(&self) -> usize {
        match self {
            SignalAction::Default => libc::SIG_DFL,
            SignalAction::Ignore => libc::SIG_IGN,
            &SignalAction::Handler(signal_handler) => match signal_handler {
                #[cfg(not(windows))]
                SignalHandler::Handler(handler) => handler as usize,
                #[cfg(not(windows))]
                SignalHandler::SigAction(sig_action) => sig_action as usize,
                #[cfg(windows)]
                SignalHandler::Address(addr) => addr,
            },
            SignalAction::DoNothing => SIG_DO_NOTHING as usize,
            SignalAction::Machine(_) => SIG_NOTIFY_MACHINE as usize,
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum MachineAction {
    Terminate,
    // how do we handle sigsegv/sigill/sigbus/sigsys?
    // we can't simply continue execution as we are in a bad/unexpected/invalid state
    // either we need to generally block this signal and handle it on a separate state or
    // or we need to use something like siglongjmp to carefully handle it in a stack frame further up the stack
    // Dump,
    Debug,
    #[allow(dead_code, reason = "not yet implemented")]
    RunGoal(/* A goal taking a signal as argument, needs to be Send */),
}

// skips signals that cannot be caught/handled/overridden
#[allow(dead_code, reason = "not all targets support all signals")]
#[derive(Debug, Clone, Copy)]
pub enum Signal {
    Hup,
    Int,
    Quit,
    Ill,
    Trap,
    Abrt,
    Iot,
    Bus,
    Emt,
    Fpe,
    // Kill,
    Usr1,
    Segv,
    Usr2,
    Pipe,
    Alrm,
    Term,
    StkFlt,
    Chld,
    Cld,
    Cont,
    // Stop,
    TStp,
    TTIn,
    TTOu,
    Urg,
    XCpu,
    XFsz,
    VtAlrm,
    Prof,
    Winch,
    Io,
    Poll,
    Pwr,
    Info,
    Lost,
    Sys,
    // Unused, // no longer defined, previously synonymous with Sys
    Other(c_int),
}

impl Signal {
    const KNOWN_SIGNALS: &[Self] = {
        #[cfg(windows)]
        {
            // https://learn.microsoft.com/en-us/cpp/c-runtime-library/reference/signal?view=msvc-180#remarks
            &[
                Signal::Abrt,
                Signal::Fpe,
                Signal::Ill,
                Signal::Int,
                Signal::Segv,
                Signal::Term,
            ]
        }
        #[cfg(not(windows))]
        {
            &[
                Signal::Hup,
                Signal::Int,
                Signal::Quit,
                Signal::Ill,
                Signal::Trap,
                Signal::Abrt,
                Signal::Iot,
                Signal::Bus,
                Signal::Fpe,
                Signal::Usr1,
                Signal::Segv,
                Signal::Usr2,
                Signal::Pipe,
                Signal::Alrm,
                Signal::Term,
                Signal::StkFlt,
                Signal::Chld,
                Signal::Cont,
                Signal::TStp,
                Signal::TTIn,
                Signal::TTOu,
                Signal::Urg,
                Signal::XCpu,
                Signal::XFsz,
                Signal::VtAlrm,
                Signal::Prof,
                Signal::Winch,
                Signal::Io,
                Signal::Poll,
                Signal::Pwr,
                Signal::Sys,
            ]
        }
    };

    #[expect(dead_code, reason = "on_signal/3 not yet implemented")]
    pub(crate) fn from_atom(atom: Atom) -> Option<Self> {
        Some(match atom {
            atom!("hup") => Self::Hup,
            atom!("int") => Self::Int,
            atom!("quit") => Self::Quit,
            atom!("ill") => Self::Ill,
            atom!("trap") => Self::Trap,
            atom!("abrt") => Self::Abrt,
            atom!("iot") => Self::Iot,
            atom!("bus") => Self::Bus,
            atom!("emt") => Self::Emt,
            atom!("fpe") => Self::Fpe,
            atom!("usr1") => Self::Usr1,
            atom!("segv") => Self::Segv,
            atom!("usr2") => Self::Usr2,
            atom!("pipe") => Self::Pipe,
            atom!("alrm") => Self::Alrm,
            atom!("term") => Self::Term,
            atom!("stkflt") => Self::StkFlt,
            atom!("chld") => Self::Chld,
            atom!("cld") => Self::Cld,
            atom!("cont") => Self::Cont,
            atom!("tStp") => Self::TStp,
            atom!("ttin") => Self::TTIn,
            atom!("ttou") => Self::TTOu,
            atom!("urg") => Self::Urg,
            atom!("xcpu") => Self::XCpu,
            atom!("xfsz") => Self::XFsz,
            atom!("vtalrm") => Self::VtAlrm,
            atom!("prof") => Self::Prof,
            atom!("winch") => Self::Winch,
            atom!("io") => Self::Io,
            atom!("poll") => Self::Poll,
            atom!("pwr") => Self::Pwr,
            atom!("info") => Self::Info,
            atom!("lost") => Self::Lost,
            atom!("sys") => Self::Sys,
            _ => return None,
        })
    }

    fn setup_signal(self) -> Result<(), std::io::Error> {
        // based on swipls default signal configuration https://www.swi-prolog.org/pldoc/man?section=signal
        let action = match self {
            Signal::Int | Signal::Info => SignalAction::Machine(MachineAction::Debug),
            Signal::Usr2 => SignalAction::DoNothing,
            Signal::Pipe => SignalAction::Ignore,
            Signal::Hup | Signal::Term | Signal::Abrt | Signal::Quit => {
                SignalAction::Machine(MachineAction::Terminate)
            }
            /*
            before we can use Dump
            Signal::Segv | Signal::Ill | Signal::Bus | Signal::Sys => {
                SignalAction::Machine(MachineAction::Dump)
            }
             */
            _ => return Ok(()),
        };
        set_signal_action(self, action)?;
        Ok(())
    }

    fn signal_id(self) -> Option<c_int> {
        match self {
            #[cfg(not(windows))]
            Signal::Hup => Some(libc::SIGHUP),
            Signal::Int => Some(libc::SIGINT),
            #[cfg(not(windows))]
            Signal::Quit => Some(libc::SIGQUIT),
            Signal::Ill => Some(libc::SIGILL),
            #[cfg(not(windows))]
            Signal::Trap => Some(libc::SIGTRAP),
            Signal::Abrt => Some(libc::SIGABRT),
            #[cfg(not(windows))]
            Signal::Iot => Some(libc::SIGIOT),
            #[cfg(not(windows))]
            Signal::Bus => Some(libc::SIGBUS),
            #[cfg(false)]
            Signal::Emt => Some(libc::SIGEMT),
            Signal::Fpe => Some(libc::SIGFPE),
            #[cfg(not(windows))]
            Signal::Usr1 => Some(libc::SIGUSR1),
            Signal::Segv => Some(libc::SIGSEGV),
            #[cfg(not(windows))]
            Signal::Usr2 => Some(libc::SIGUSR2),
            #[cfg(not(windows))]
            Signal::Pipe => Some(libc::SIGPIPE),
            #[cfg(not(windows))]
            Signal::Alrm => Some(libc::SIGALRM),
            Signal::Term => Some(libc::SIGTERM),
            #[cfg(not(windows))]
            Signal::StkFlt => Some(libc::SIGSTKFLT),
            #[cfg(not(windows))]
            Signal::Chld => Some(libc::SIGCHLD),
            #[cfg(false)]
            Signal::Cld => Some(libc::SIGCLD),
            #[cfg(not(windows))]
            Signal::Cont => Some(libc::SIGCONT),
            #[cfg(not(windows))]
            Signal::TStp => Some(libc::SIGTSTP),
            #[cfg(not(windows))]
            Signal::TTIn => Some(libc::SIGTTIN),
            #[cfg(not(windows))]
            Signal::TTOu => Some(libc::SIGTTOU),
            #[cfg(not(windows))]
            Signal::Urg => Some(libc::SIGURG),
            #[cfg(not(windows))]
            Signal::XCpu => Some(libc::SIGXCPU),
            #[cfg(not(windows))]
            Signal::XFsz => Some(libc::SIGXFSZ),
            #[cfg(not(windows))]
            Signal::VtAlrm => Some(libc::SIGVTALRM),
            #[cfg(not(windows))]
            Signal::Prof => Some(libc::SIGPROF),
            #[cfg(not(windows))]
            Signal::Winch => Some(libc::SIGWINCH),
            #[cfg(not(windows))]
            Signal::Io => Some(libc::SIGIO),
            #[cfg(not(windows))]
            Signal::Poll => Some(libc::SIGPOLL),
            #[cfg(not(windows))]
            Signal::Pwr => Some(libc::SIGPWR),
            #[cfg(false)]
            Signal::Info => Some(libc::SIGINFO),
            #[cfg(false)]
            Signal::Lost => Some(libc::SIGLOST),
            #[cfg(not(windows))]
            Signal::Sys => Some(libc::SIGSYS),
            Signal::Other(signal_number) => Some(signal_number),
            _ => None,
        }
    }

    fn from_number(signal_number: i32) -> Signal {
        match signal_number {
            libc::SIGINT => Self::Int,
            libc::SIGILL => Self::Ill,
            libc::SIGABRT => Self::Abrt,
            libc::SIGFPE => Self::Fpe,
            libc::SIGSEGV => Self::Segv,
            libc::SIGTERM => Self::Term,
            _ => Self::Other(signal_number),
        }
    }

    pub(crate) fn into_literal(self) -> Literal {
        match self {
            Signal::Hup => Literal::Atom(atom!("hup")),
            Signal::Int => Literal::Atom(atom!("int")),
            Signal::Quit => Literal::Atom(atom!("quit")),
            Signal::Ill => Literal::Atom(atom!("ill")),
            Signal::Trap => Literal::Atom(atom!("trap")),
            Signal::Abrt => Literal::Atom(atom!("abrt")),
            Signal::Iot => Literal::Atom(atom!("iot")),
            Signal::Bus => Literal::Atom(atom!("bus")),
            Signal::Emt => Literal::Atom(atom!("emt")),
            Signal::Fpe => Literal::Atom(atom!("fpe")),
            Signal::Usr1 => Literal::Atom(atom!("usr1")),
            Signal::Segv => Literal::Atom(atom!("segv")),
            Signal::Usr2 => Literal::Atom(atom!("usr2")),
            Signal::Pipe => Literal::Atom(atom!("pipe")),
            Signal::Alrm => Literal::Atom(atom!("alrm")),
            Signal::Term => Literal::Atom(atom!("term")),
            Signal::StkFlt => Literal::Atom(atom!("stkflt")),
            Signal::Chld => Literal::Atom(atom!("chld")),
            Signal::Cld => Literal::Atom(atom!("cld")),
            Signal::Cont => Literal::Atom(atom!("cont")),
            Signal::TStp => Literal::Atom(atom!("tStp")),
            Signal::TTIn => Literal::Atom(atom!("ttin")),
            Signal::TTOu => Literal::Atom(atom!("ttou")),
            Signal::Urg => Literal::Atom(atom!("urg")),
            Signal::XCpu => Literal::Atom(atom!("xcpu")),
            Signal::XFsz => Literal::Atom(atom!("xfsz")),
            Signal::VtAlrm => Literal::Atom(atom!("vtalrm")),
            Signal::Prof => Literal::Atom(atom!("prof")),
            Signal::Winch => Literal::Atom(atom!("winch")),
            Signal::Io => Literal::Atom(atom!("io")),
            Signal::Poll => Literal::Atom(atom!("poll")),
            Signal::Pwr => Literal::Atom(atom!("pwr")),
            Signal::Info => Literal::Atom(atom!("info")),
            Signal::Lost => Literal::Atom(atom!("lost")),
            Signal::Sys => Literal::Atom(atom!("sys")),
            Signal::Other(idx) => Literal::Fixnum(Fixnum::build_with(idx)),
        }
    }
}

// if non-zero a signal has occurred, the machine should perform the action configured for each signal as indicated by the set bits
// i.e. if bit n is set the action for signal (n+1)
// FIXME support unix real-time signals
static PENDING_SIGNALS: AtomicU32 = AtomicU32::new(0);

// using static to ensure a fixed address
static SIG_DO_NOTHING: extern "C" fn(c_int) = {
    extern "C" fn do_nothing(_: c_int) {
        // interrupts syscals but does nothing otherwise
    }
    do_nothing
};

// using static to ensure a fixed address
static SIG_NOTIFY_MACHINE: extern "C" fn(c_int) = {
    extern "C" fn mark_pending(signal: c_int) {
        // mark the signal as pending if we are able to
        if 0 < signal && signal as u32 <= u32::BITS {
            PENDING_SIGNALS.fetch_or(1 << (signal - 1), atomic::Ordering::Release);
        }
    }
    mark_pending
};

static EDIT_SIGNAL: LazyLock<Mutex<Vec<SignalAction>>> = LazyLock::new(|| Mutex::new(vec![]));

#[expect(dead_code, reason = "current_signal/3 not yet implemented")]
pub(crate) fn get_current_signal_action(signal: Signal) -> Option<SignalAction> {
    let signal_id = signal.signal_id()?;
    #[cfg(windows)]
    {
        // https://learn.microsoft.com/de-at/cpp/c-runtime-library/signal-action-constants?view=msvc-180

        let current = unsafe { libc::signal(signal_id, libc::SIG_GET) };
        Some(match current {
            libc::SIG_DFL => SignalAction::Default,
            libc::SIG_IGN => SignalAction::Ignore,
            _ if current == SIG_NOTIFY_MACHINE as usize => {
                // index should be inbounds as SIG_NOTIFY_MACHINE is configured as the signal handler
                EDIT_SIGNAL.lock().ok()?[signal_id as usize].clone()
            }
            _ => SignalAction::Handler(SignalHandler::Address(current)),
        })
    }
    #[cfg(not(windows))]
    {
        use libc::sigaction;
        use std::mem::MaybeUninit;

        let mut oldact: sigaction = unsafe { MaybeUninit::zeroed().assume_init() };

        let res = unsafe { libc::sigaction(signal_id, core::ptr::null(), &mut oldact) };
        if res != 0 {
            return None;
        }
        Some(match oldact.sa_sigaction {
            libc::SIG_DFL => SignalAction::Default,
            libc::SIG_IGN => SignalAction::Ignore,
            current if current == SIG_NOTIFY_MACHINE as usize => {
                // index should be inbounds as SIG_NOTIFY_MACHINE is configured as the signal handler
                EDIT_SIGNAL.lock().ok()?[signal_id as usize].clone()
            }
            current => {
                use libc::SA_SIGINFO;

                if oldact.sa_flags & SA_SIGINFO != 0 {
                    SignalAction::Handler(SignalHandler::SigAction(unsafe {
                        std::mem::transmute::<
                            *const (),
                            unsafe extern "C" fn(i32, *const libc::siginfo_t, *const libc::c_void),
                        >(current as *const ())
                    }))
                } else {
                    SignalAction::Handler(SignalHandler::Handler(unsafe {
                        std::mem::transmute::<*const (), unsafe extern "C" fn(i32)>(
                            current as *const (),
                        )
                    }))
                }
            }
        })
    }
}

pub fn set_signal_action(
    signal: Signal,
    action: SignalAction,
) -> Result<SignalAction, std::io::Error> {
    let signal_number = signal.signal_id().ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::Unsupported,
            "Provided signal unavailable on this system",
        )
    })?;
    let signal_idx = usize::try_from(signal_number)
        .map_err(|_err| std::io::Error::other("Signal number not in range for usize"))?;

    // prevent concurrent edits of signals
    let mut guard = EDIT_SIGNAL
        .lock()
        .map_err(|_err| std::io::Error::other("lock is poisoned"))?;

    if let SignalAction::Machine(_) = action {
        // to guarantee that we are able to set the state when updating the signal succeeds
        // ensure the list is large enough before we attempt set the signal handler
        let len = guard.len();
        if len <= signal_idx {
            guard.try_reserve(signal_idx - len + 1)?;
            guard.resize(signal_idx + 1, SignalAction::Default);
        }
    }

    let old_action;

    #[cfg(windows)]
    {
        let new_handler = action.to_handler_address();
        let old_handler = unsafe { libc::signal(signal_number, new_handler) };

        let old_entry = if signal_idx < guard.len() {
            std::mem::replace(&mut guard[signal_idx], action)
        } else {
            SignalAction::Default
        };

        old_action = match old_handler {
            libc::SIG_DFL => SignalAction::Default,
            libc::SIG_IGN => SignalAction::Ignore,
            old if old == SIG_NOTIFY_MACHINE as usize => old_entry,
            // other signal handlers including SIG_DO_NOTHING
            old => SignalAction::Handler(SignalHandler::Address(old)),
        };
    }

    #[cfg(not(windows))]
    {
        use std::mem::MaybeUninit;

        let mut maybe_oldact = MaybeUninit::<libc::sigaction>::uninit();
        let newact = action.to_action();

        if unsafe { libc::sigaction(signal_number, &newact, maybe_oldact.as_mut_ptr()) } != 0 {
            return Err(std::io::Error::last_os_error());
        }

        let old_entry = if signal_idx < guard.len() {
            std::mem::replace(&mut guard[signal_idx], action)
        } else {
            SignalAction::Default
        };

        let oldact = unsafe { maybe_oldact.assume_init_ref() };

        old_action = match oldact.sa_sigaction {
            libc::SIG_DFL => SignalAction::Default,
            libc::SIG_IGN => SignalAction::Ignore,
            old if old == SIG_NOTIFY_MACHINE as usize => old_entry,
            // other signal handlers including SIG_DO_NOTHING
            old => {
                use libc::SA_SIGINFO;

                if oldact.sa_flags & SA_SIGINFO != 0 {
                    SignalAction::Handler(SignalHandler::SigAction(unsafe {
                        std::mem::transmute::<
                            *const (),
                            unsafe extern "C" fn(i32, *const libc::siginfo_t, *const libc::c_void),
                        >(old as *const ())
                    }))
                } else {
                    SignalAction::Handler(SignalHandler::Handler(unsafe {
                        std::mem::transmute::<*const (), unsafe extern "C" fn(i32)>(
                            old as *const (),
                        )
                    }))
                }
            }
        };
    }

    drop(guard);

    Ok(old_action)
}

pub fn setup_default_signal_handlers() {
    for signal in Signal::KNOWN_SIGNALS {
        if let Err(err) = signal.setup_signal() {
            eprintln!("Failed to setup signal {signal:?}: {err}");
        }
    }

    #[cfg(windows)]
    {
        // https://learn.microsoft.com/en-us/windows/console/setconsolectrlhandler?redirectedfrom=MSDN
        use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;

        if unsafe {
            SetConsoleCtrlHandler(Some(os_handler), 1 /* True */)
        } == 0
        {
            let err = std::io::Error::last_os_error();
            eprintln!("Failed to add handler with SetConsoleCtrlHandler: {err}");
        } else {
            if cfg!(debug_assertions) {
                println!("Setup SetConsoleCtrlHandler handler");
            }
        }
    }
}

#[cfg(windows)]
extern "system" fn os_handler(event: u32) -> windows_sys::core::BOOL {
    use windows_sys::Win32::System::Console::{
        CTRL_BREAK_EVENT, CTRL_CLOSE_EVENT, CTRL_C_EVENT, CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT,
    };
    // https://learn.microsoft.com/en-us/windows/console/setconsolectrlhandler?redirectedfrom=MSDN
    match event {
        CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT | CTRL_C_EVENT | CTRL_LOGOFF_EVENT
        | CTRL_SHUTDOWN_EVENT => {
            PENDING_SIGNALS.fetch_or(1 << (libc::SIGINT - 1), atomic::Ordering::Relaxed);
            1 // True
        }
        _ => {
            // unknown event
            0 // False
        }
    }
}

#[derive(Debug)]
pub(crate) struct PendingSignal {
    pub(crate) signal: Signal,
    pub(crate) action: SignalAction,
}

pub(crate) fn next_pending_signal() -> Option<PendingSignal> {
    let guard = EDIT_SIGNAL.lock().ok()?;
    let mut pending = PENDING_SIGNALS.load(atomic::Ordering::Relaxed);
    loop {
        if pending == 0 {
            return None;
        }

        // isolate the first one bit
        let first_one = ((!pending) + 1) & pending;
        // clear that bit from PENDING_SIGNALS
        pending = PENDING_SIGNALS.fetch_and(!first_one, atomic::Ordering::Relaxed);

        // check that no other thread cleared the signal first otherwise try again
        if pending & first_one != 0 {
            let signal_number = first_one.ilog2() + 1;
            return Some(PendingSignal {
                signal: Signal::from_number(signal_number as i32),
                action: guard.get(signal_number as usize)?.clone(),
            });
        }
    }
}
