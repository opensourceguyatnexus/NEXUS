# Contributing to NEXUS

NEXUS is open source, and anyone can help. You don't need to be a programmer or a gamer.

## Ways to help

- **Report a bug:** [open an issue](https://github.com/opensourceguyatnexus/NEXUS/issues/new) with what you did, what you expected, what happened, and your Windows version.
- **Suggest an idea:** open an issue and describe the problem you want solved.
- **Test launchers:** Riot Games, Epic Games and Ubisoft Connect switching needs more real-world testing. Tell us what worked.
- **Improve the docs or website:** the site is in `website/`.
- **Send code:** fork the repo, make your change on a branch, and open a pull request.

## Run it locally

You need [Node.js](https://nodejs.org) and [Rust](https://rustup.rs).

```
npm install
npm run tauri dev
```

Before you open a pull request, make sure these pass:

```
npx tsc --noEmit
cd src-tauri
cargo test --lib
```

## Stay safe while testing

- **Never commit login data.** Saved accounts live in `%LOCALAPPDATA%\NEXUS\accounts`. Don't paste those files, tokens or passwords into issues, logs or commits.
- **Test Disk Manager on a spare USB drive,** never on a drive with files you care about.
- **Keep the guards.** Protected partitions and processes, and the typed confirmations, are there for a reason.

## Pull requests

Keep each one focused on one change, and explain what it does and how you tested it. Be kind and assume good intent.

By contributing, you agree that your work is shared under the [MIT License](LICENSE).
