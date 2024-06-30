# DiscordTavern

A [SillyTavern](https://github.com/SillyTavern/SillyTavern)-inspired Discord bot for chatting with characters.

# Features

- Message swiping, editing, pinning
- Character creation, editing, and deleting
- Slash and prefix commands
- Text streaming (1s intervals, due to Discord's rate limits)
- Multi-user aware

# Running

On startup, `config.ron` will be created; set your `bot_id`, `bot_token`, and `openai_key` here. 

# Configuration

`config.ron` is created on startup. Bring your own `bot_id`, `bot_token`, and `openai_key`, and optionally your own `openai_url` and `openai_model`. `name_substitutes` is a list of pairs of strings; the first name will be swapped out for the second. For example, the Discord username (not display name) `bobgamer123` could be swapped out for `Bob`, or anything else, really.

# Building

`cargo build [--release]`

# Notes

Currently, the code is not particularly good. Additionally, nothing is documented, nor are there any comments, and the user-facing text is in Swedish. This is all likely to change.
