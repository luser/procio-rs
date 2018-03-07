Run a process on Linux and track its I/O throughput.

run `procio command arguments...` and it will print stats every second.

Pass `-o FILE` to redirect the command's stdout to `FILE`.

Sample Output
=============

```
$ cargo run -- -o /dev/null /usr/bin/yes
   Compiling procio v0.1.0 (file:///build/procio-rs)
    Finished dev [unoptimized + debuginfo] target(s) in 1.24 secs
     Running `target/debug/procio -o /dev/null /usr/bin/yes`
1.000 s: 2 KiB/s read, 29 GiB/s write
2.000 s: 0 B/s read, 29 GiB/s write
3.000 s: 0 B/s read, 28 GiB/s write
4.000 s: 0 B/s read, 28 GiB/s write
^C
```
