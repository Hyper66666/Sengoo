use super::{
    clear_poll_wakeup_hint, merge_wakeup_hints, sengoo_async_poll_dispatch,
    sengoo_async_result_dispatch_bool, sengoo_async_result_dispatch_f32,
    sengoo_async_result_dispatch_f64, sengoo_async_result_dispatch_i16,
    sengoo_async_result_dispatch_i32, sengoo_async_result_dispatch_i64,
    sengoo_async_result_dispatch_i8, take_poll_wakeup_hint, wait_for_wakeup_hint_or_yield,
};

const SELECT_WINNER_FIRST: i64 = 0;
const SELECT_WINNER_SECOND: i64 = 1;

unsafe fn wait_for_first_ready_winner(
    first_kind: i64,
    first_handle: i64,
    second_kind: i64,
    second_handle: i64,
) -> i64 {
    loop {
        clear_poll_wakeup_hint();
        if sengoo_async_poll_dispatch(first_kind, first_handle) != 0 {
            return SELECT_WINNER_FIRST;
        }
        let first_hint = take_poll_wakeup_hint();

        clear_poll_wakeup_hint();
        if sengoo_async_poll_dispatch(second_kind, second_handle) != 0 {
            return SELECT_WINNER_SECOND;
        }
        let second_hint = take_poll_wakeup_hint();

        wait_for_wakeup_hint_or_yield(merge_wakeup_hints(first_hint, second_hint));
    }
}

unsafe fn wait_for_first_ready<T>(
    first_kind: i64,
    first_handle: i64,
    second_kind: i64,
    second_handle: i64,
    dispatch: unsafe extern "C" fn(i64, i64) -> T,
) -> T {
    match wait_for_first_ready_winner(first_kind, first_handle, second_kind, second_handle) {
        SELECT_WINNER_FIRST => dispatch(first_kind, first_handle),
        SELECT_WINNER_SECOND => dispatch(second_kind, second_handle),
        winner => unreachable!("unexpected async select winner: {winner}"),
    }
}

macro_rules! define_async_select {
    ($name:ident, $dispatch:path, $ret:ty) => {
        #[no_mangle]
        pub extern "C" fn $name(
            first_kind: i64,
            first_handle: i64,
            second_kind: i64,
            second_handle: i64,
        ) -> $ret {
            unsafe {
                wait_for_first_ready(
                    first_kind,
                    first_handle,
                    second_kind,
                    second_handle,
                    $dispatch,
                )
            }
        }
    };
}

#[no_mangle]
pub extern "C" fn sengoo_async_select_winner(
    first_kind: i64,
    first_handle: i64,
    second_kind: i64,
    second_handle: i64,
) -> i64 {
    unsafe { wait_for_first_ready_winner(first_kind, first_handle, second_kind, second_handle) }
}

define_async_select!(sengoo_async_select_i8, sengoo_async_result_dispatch_i8, i8);
define_async_select!(
    sengoo_async_select_i16,
    sengoo_async_result_dispatch_i16,
    i16
);
define_async_select!(
    sengoo_async_select_i32,
    sengoo_async_result_dispatch_i32,
    i32
);
define_async_select!(
    sengoo_async_select_i64,
    sengoo_async_result_dispatch_i64,
    i64
);
define_async_select!(
    sengoo_async_select_bool,
    sengoo_async_result_dispatch_bool,
    bool
);
define_async_select!(
    sengoo_async_select_f32,
    sengoo_async_result_dispatch_f32,
    f32
);
define_async_select!(
    sengoo_async_select_f64,
    sengoo_async_result_dispatch_f64,
    f64
);
