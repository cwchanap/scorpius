# Scorpius task runner. Run `just` (no args) to list recipes.

# List available recipes (default)
default:
    @just --list

# Launch the game (Title screen)
run:
    cargo run
