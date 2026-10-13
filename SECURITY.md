# Security policy

## Reporting a vulnerability

Please report security issues privately through GitHub: open the repository's
**Security** tab and choose **Report a vulnerability**. Don't file a public
issue for a security problem.

Include what you found, how to reproduce it, and the affected version or
commit. You'll get an acknowledgement within a few days, and we'll keep you
updated until it's fixed.

## Supported versions

Volt UI Kit is pre-1.0. Security fixes go into the latest release only.

## Scope

The toolkit validates and bounds the input it processes: interface trees, JSON
documents, images, fonts, SVG files and accessibility requests. See
[Architecture](docs/architecture.md#security-and-resource-boundaries) for the
limits. Your application's callbacks, file access and network code are outside
its control.
