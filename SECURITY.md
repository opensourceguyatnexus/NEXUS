# Security

NEXUS saves launcher logins on your PC and can run disk and process commands with administrator rights, so security reports are taken seriously.

## Report a problem privately

Please don't open a public issue for a security problem. Instead, open the [Security tab](https://github.com/opensourceguyatnexus/NEXUS/security) of this repository and choose **Report a vulnerability**. Only the maintainers can see the report.

Include what you found, the steps to reproduce it, and your NEXUS and Windows versions. Never include real tokens, passwords or files from `%LOCALAPPDATA%\NEXUS\accounts`.

## What counts

- Saved account data being read, changed or leaked by something that shouldn't have access
- NEXUS running something with administrator rights that the user didn't ask for
- Ways around the guards on protected partitions and processes, or around the typed confirmations
- The exported installer or an update running something other than what the app shows

Ordinary bugs, such as a launcher that doesn't switch or a button that does nothing, belong in a normal [issue](https://github.com/opensourceguyatnexus/NEXUS/issues/new/choose).

## Supported versions

Fixes go into the latest release. Please update before reporting.
