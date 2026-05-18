# Async Examples

Run each file with `sgc run <path>`.

| File | Demonstrates | Expected output |
|---|---|---:|
| [`01_sleep_spawn.sg`](01_sleep_spawn.sg) | Spawn a sleeping child future and await it | `42` |
| [`02_select_two.sg`](02_select_two.sg) | Select the first completed result from two futures | `43` |
| [`03_spawn_task_lifecycle.sg`](03_spawn_task_lifecycle.sg) | `spawn_task`, `task_status`, and completion checks | `42` |
