# Security

## Custom Apply Scripts

`apply.backend=custom-script` executes `apply.custom_script` as trusted user code.
`walls` does not sandbox, chroot, drop privileges, filter environment variables, or
apply filesystem/network restrictions to the script.

The script runs as the same user account that invoked `walls`, `walls-tray`, or the
systemd user service. Treat the configured script with the same trust level as any
other executable in your login session.

Before running a custom script, `walls config validate` checks that:

- `apply.custom_script` is set when `apply.backend` is `custom-script`.
- the configured path exists and is a file.
- on Unix, the file has at least one executable permission bit.

Validation does not prove the script is safe. Review the script yourself before
configuring it.

### Script Arguments

`walls` starts the script directly, not through a shell, and passes four positional
arguments:

| Position | Value | Notes |
| --- | --- | --- |
| `$1` | display path | The composed wallpaper path. For COSMIC configs using `use_original_path`, this may be the original path. |
| `$2` | trigger | One of `auto`, `manual`, or `refresh`. |
| `$3` | original path | The source wallpaper path before display-mode composition. |
| `$4` | fill mode | One of `zoom`, `spanned`, `centered`, `scaled`, `stretched`, `wallpaper`, or `os`. |

`walls` does not add custom environment variables for the script. The child process
inherits the parent process environment from the CLI, tray, or systemd user service
that launched it.

### Example

```sh
#!/bin/sh
set -eu

display_path=$1
trigger=$2
original_path=$3
fill_mode=$4

printf 'applying %s from %s (%s, %s)\n' \
  "$display_path" "$original_path" "$trigger" "$fill_mode"

feh --bg-fill "$display_path"
```
