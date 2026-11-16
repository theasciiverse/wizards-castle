# Wizard's Castle, ncurses version

This is a full-screen terminal front end on the [Wizard's Castle
backend](https://github.com/beejjorgensen/Wizards-Castle-Rust). (The backend
also comes with a non-curses classic version of the game.)

It started out on ncurses and now draws with
[Ratatui](https://ratatui.rs/) and crossterm, so it runs on Linux, macOS,
and Windows, in any terminal of 80x24 or more, and follows the terminal
when it is resized. But it's still full of curses goodness:

![Screenshot](img/wizcas-screen1.png)

* [More Wizard's Castle general info](https://github.com/beejjorgensen/Wizards-Castle-Info)

## TODO

* Bold on all key prompts? Or on none?
