use super::{
    clear_poll_wakeup_hint, merge_wakeup_hints, sengoo_async_poll_dispatch,
    sengoo_async_result_dispatch_bool, sengoo_async_result_dispatch_f32,
    sengoo_async_result_dispatch_f64, sengoo_async_result_dispatch_i16,
    sengoo_async_result_dispatch_i32, sengoo_async_result_dispatch_i64,
    sengoo_async_result_dispatch_i8, take_poll_wakeup_hint, wait_for_wakeup_hint_or_yield,
};

const MAX_SELECT_OPERANDS: usize = 8;

unsafe fn wait_for_first_ready_n(operands: &[(i64, i64)]) -> usize {
    let count = operands.len();
    debug_assert!((2..=MAX_SELECT_OPERANDS).contains(&count));

    let mut rotation = 0usize;
    loop {
        let mut merged_hint = None;

        for offset in 0..count {
            let index = (rotation + offset) % count;
            let (kind, handle) = operands[index];

            clear_poll_wakeup_hint();
            if sengoo_async_poll_dispatch(kind, handle) != 0 {
                return index;
            }
            let hint = take_poll_wakeup_hint();
            merged_hint = merge_wakeup_hints(merged_hint, hint);
        }

        rotation = (rotation + 1) % count;
        wait_for_wakeup_hint_or_yield(merged_hint);
    }
}

unsafe fn wait_for_first_ready_winner(
    first_kind: i64,
    first_handle: i64,
    second_kind: i64,
    second_handle: i64,
) -> i64 {
    wait_for_first_ready_n(&[(first_kind, first_handle), (second_kind, second_handle)]) as i64
}

unsafe fn wait_for_first_ready<T>(
    first_kind: i64,
    first_handle: i64,
    second_kind: i64,
    second_handle: i64,
    dispatch: unsafe extern "C" fn(i64, i64) -> T,
) -> T {
    let winner = wait_for_first_ready_winner(first_kind, first_handle, second_kind, second_handle);
    match winner {
        0 => dispatch(first_kind, first_handle),
        1 => dispatch(second_kind, second_handle),
        winner => unreachable!("unexpected async select winner: {winner}"),
    }
}

#[no_mangle]
pub extern "C" fn sengoo_async_select_n_winner(
    count: i64,
    kind0: i64,
    handle0: i64,
    kind1: i64,
    handle1: i64,
    kind2: i64,
    handle2: i64,
    kind3: i64,
    handle3: i64,
    kind4: i64,
    handle4: i64,
    kind5: i64,
    handle5: i64,
    kind6: i64,
    handle6: i64,
    kind7: i64,
    handle7: i64,
) -> i64 {
    let count = count as usize;
    if !(2..=MAX_SELECT_OPERANDS).contains(&count) {
        return 0;
    }

    let pairs = [
        (kind0, handle0),
        (kind1, handle1),
        (kind2, handle2),
        (kind3, handle3),
        (kind4, handle4),
        (kind5, handle5),
        (kind6, handle6),
        (kind7, handle7),
    ];

    unsafe { wait_for_first_ready_n(&pairs[..count]) as i64 }
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
