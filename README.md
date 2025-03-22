# Mintaka: Run long-running processes in parallel, automatically focus on problems

Mintaka runs long-running processes in parallel, and automatically focuses on
problems in any of those processes.

For instance, while you're developing a web application, suppose you want to run
several processes at once using tmux:

* The server
* Cosmos
* esbuild
* The TypeScript compiler (tsc)
* eslint

You could run each of these in their own tmux window, but this makes errors easy
to miss and cumbersome to check. If you run each in their own tmux pane, you can
see when they go wrong, but the space for each pane is limited, and it's often
quite useful to have a large pane if, say, there's a compilation error.

Mintaka shows the status of all the processes at once, while automatically
focusing on the first process that has a problem, allowing you to solve that
problem before moving onto the next problem.

For instance, running `mintaka -c config.toml` when there's a TypeScript error:

![A screenshot of Mintaka, showing a list of processes down the left-hand side
with tsc highlighted, and the output of tsc on the right-hand side
](screenshot.png?raw=true)

where `config.toml` is:

```toml
[[processes]]
name = "Server"
command = ["npm", "start"]

[[processes]]
name = "Cosmos"
command = ["npm", "run", "cosmos"]

[[processes]]
name = "Build"
command = ["npm", "run", "build-watch"]
error_regex = "Build finished with ([0-9]+) errors"

[[processes]]
name = "tsc"
command = ["npm", "run", "check:tsc", "--", "--watch"]
type = "tsc-watch"

[[processes]]
name = "eslint"
command = ["npm", "run", "check:eslint"]
after = "tsc"
```

Note that Mintaka is still under early development: there are likely bugs,
performance is probably quite poor, and the config file format might change.

## Configuration

Mintaka is configured using a TOML file that should have a `processes` array,
with each process having the keys:

* `command`: An array of strings describing how to start the process.

* `working_directory`: Optionally, the working directory that the process should
  initially have.

* `name`: Optionally, a string that is used to describe the process. If not set,
  a name will be automatically generated from the command.

* `after`: Optionally, the name of another process. Whenever that other process
  reaches a successful state, this process will be restarted. If `after` is set,
  the process will not be automatically started when Mintaka starts.

* `autostart`: Optionally, whether process should be automatically started when
  Mintaka starts. If not set, this defaults to `true` unless `after` is set.

* `type`: Optionally, the type of process that is running. This determines how
  Mintaka detects the current status of a running process for common
  executables.

  Currently only `tsc-watch` is supported, which handles `tsc --watch` commands.

* `error_regex`: Optionally, a regex that can be applied to each line of the
  output of a process to determine its status. When the regex matches:

  * If the regex has no capture groups, the process will have a status of
    "Error".

  * If the regex has at least one capture group, the first capture group will be
    used as the error count. If the error count is zero, the process will have a
    status of "Success", otherwise the process will have a status of "Error".

* `success_regex`: Optionally, a regex that can be applied to each line of the
  output of a process to determine its status. If it matches, the process will
  have a status of "Success".

## Statuses

A process can have the following statuses:

* Inactive: the process has not started.

* Running: the process is running and has not yet reached a success or error
  state. For instance, `tsc --watch` will be in this state during compilation.

* Success: the process is running and has reached a success state. For instance,
  `tsc --watch` will be in this state if the last printed line was
  `Found 0 errors. Watching for file changes.`.

* Error: the process is running and has reached a success state. For instance,
  `tsc --watch` will be in this state if the last printed line was
  `Found 2 errors. Watching for file changes.`.

* Exited: the process has exited.

For the purposes of starting other processes, the successful statuses are
"Success" and "Exited" when the exit code is 0.

## Keyboard shortcuts

* Press `a` to toggle autofocus. When autofocus is on, the first process with
  an error will be focused automatically.
* Use the up and down arrow keys to focus on the previous and next process
  respectively.
* Press `r` to restart the focused process.
* Press `Ctrl+e` to enter a process. While a process is entered, any input will
  be sent to that process. Press `Ctrl+e` again to leave the process.
* Press `Ctrl+c` to quit.
