`Repository in early stages of development.`

## Repo Overview

**My-Sourcecode** is a TUI binary that uses established packages to scan and analyze your repository’s source code.

## Core Features

- Shows the number of lines of code, blank lines, and comments in your project.
- Lists all files and their programming languages.
- Filters results to show only source-code-related files, using the same approach as GitHub’s Linguist.
- Shows a repository’s language breakdown: the percentage of code written in each programming language.
- Computes the average number of lines of code per file.

## Tech Stack

| Component | Technology |
|---|---|
| Language | Rust |
| TUI framework | ratatui |
| Terminal backend | crossterm |
| Statistics library | tokei |

## How to Run

- Binary releases are currently available only for Linux x86_64 systems.
- Place the binary in a directory and make it executable.
- Optionally, create a desktop entry so you can launch it with a click.
- Alternatively, run it from a terminal in several ways.