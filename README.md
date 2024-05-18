# Mintaka: Run long-running process, automatically focus on problems

Mintaka runs long-running processes, and automatically focuses on problems in
any of those processes.

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

![A screenshot of Mintaka, showing a list of processes down the left-hand side
with tsc highlighted, and the output of tsc on the right-hand side
](screenshot.png?raw=true)

Note that Mintaka is still under early development: there are likely bugs,
performance is probably quite poor, and the config file format might change.

## Keyboard shortcuts

* Press `a` to toggle autofocus.
* Press `r` to restart the focused process.
* Use the up and down arrow keys to focus on the previous and next process
  respectively.
* Press `q` or `Ctrl+c` to quit.
