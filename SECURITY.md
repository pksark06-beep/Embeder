# Security policy

Security fixes are provided for the latest supported release and the `main`
branch. Older releases may need to be updated.

Please report a suspected vulnerability through
[GitHub private vulnerability reporting](https://github.com/pksark06-beep/Embeder/security/advisories/new).
Include the affected version or commit, a reproduction, impact, and any
suggested mitigation. Do not include API keys or other private data. If private
reporting is unavailable, open an issue asking the maintainers for a private
contact channel without posting exploit details.

Embeder compiles and simulates generated firmware locally. Its current path
checks are application-level controls; compiler and simulator child processes
are not yet contained by an operating-system sandbox. Treat projects from
untrusted sources accordingly. The desktop development server is intended for
loopback access only. Provider API keys stay on the user's machine except when
sent to the selected model endpoint.
