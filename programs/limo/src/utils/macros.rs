#[cfg(target_arch = "bpf")]
#[macro_export]
macro_rules! dbg_msg {
                () => {
        msg!("[{}:{}]", file!(), line!())
    };
    ($val:expr $(,)?) => {
                      match $val {
            tmp => {
                msg!("[{}:{}] {} = {:#?}",
                    file!(), line!(), stringify!($val), &tmp);
                tmp
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg_msg!($val)),+,)
    };
}

#[cfg(not(target_arch = "bpf"))]
#[macro_export]
macro_rules! dbg_msg {
                () => {
        println!("[{}:{}]", file!(), line!())
    };
    ($val:expr $(,)?) => {
                      match $val {
            tmp => {
                println!("[{}:{}] {} = {:#?}",
                    file!(), line!(), stringify!($val), &tmp);
                tmp
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg_msg!($val)),+,)
    };
}

#[cfg(target_arch = "bpf")]
#[macro_export]
macro_rules! xmsg {
    ($($arg:tt)*) => (msg!($($arg)*));
}

#[cfg(not(target_arch = "bpf"))]
#[macro_export]
macro_rules! xmsg {
    ($($arg:tt)*) => (println!($($arg)*));
}

#[macro_export]
macro_rules! assert_fuzzy_eq {
    ($actual:expr, $expected:expr, $epsilon:expr) => {
        let eps = $epsilon as i128;
        let act = $actual as i128;
        let exp = $expected as i128;
        let diff = (act - exp).abs();
        if diff > eps {
            panic!(
                "Actual {} Expected {} diff {} Epsilon {}",
                $actual, $expected, diff, eps
            );
        }
    };

    ($actual:expr, $expected:expr, $epsilon:expr, $type:ty) => {
        let eps = $epsilon as $type;
        let act = $actual as $type;
        let exp = $expected as $type;
        let diff = (act - exp).abs();
        if diff > eps {
            panic!(
                "Actual {} Expected {} diff {} Epsilon {}",
                $actual, $expected, diff, eps
            );
        }
    };
}
