macro_rules! single_arg {
    ($icx:ident) => {
        match &$icx.arg_values[..] {
            [(arg_nid, arg_value)] => (*arg_nid, arg_value),
            _ => {
                return Err($icx.caller_cx.err(
                    $icx.call_span,
                    crate::runner::RunnerError::IncorrectArgs {
                        required: 1..=1,
                        actual: $icx.arg_values.len(),
                    },
                ))
            }
        }
    };
}

macro_rules! two_args {
    ($icx:ident) => {
        match &$icx.arg_values[..] {
            [(arg1_nid, arg1_value), (arg2_nid, arg2_value)] => {
                ((*arg1_nid, arg1_value), (*arg2_nid, arg2_value))
            }
            _ => {
                return Err($icx.caller_cx.err(
                    $icx.call_span,
                    crate::runner::RunnerError::IncorrectArgs {
                        required: 2..=2,
                        actual: $icx.arg_values.len(),
                    },
                ))
            }
        }
    };
}

macro_rules! three_args {
    ($icx:ident) => {
        match &$icx.arg_values[..] {
            [
                (arg1_nid, arg1_value),
                (arg2_nid, arg2_value),
                (arg3_nid, arg3_value),
            ] => (
                (*arg1_nid, arg1_value),
                (*arg2_nid, arg2_value),
                (*arg3_nid, arg3_value),
            ),
            _ => {
                return Err($icx.caller_cx.err(
                    $icx.call_span,
                    crate::runner::RunnerError::IncorrectArgs {
                        required: 3..=3,
                        actual: $icx.arg_values.len(),
                    },
                ))
            }
        }
    };
}

macro_rules! expect_type {
    ($icx:expr, $typ:ident, $nid_value:expr) => {{
        let (nid, value) = $nid_value;
        let Value::$typ(v) = value else {
            return Err($icx.caller_cx.nid_err(
                nid,
                crate::runner::RunnerError::ExpectedType {
                    actual: value.rain_type_id(),
                    expected: std::borrow::Cow::Borrowed(&[crate::runner::value::RainTypeId::$typ]),
                },
            ));
        };
        debug_assert_eq!(value.rain_type_id(), crate::runner::value::RainTypeId::$typ);
        v
    }};
}

pub(crate) use expect_type;
pub(crate) use single_arg;
pub(crate) use three_args;
pub(crate) use two_args;
