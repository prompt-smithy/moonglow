# Moonglow Privacy Policy

Last updated: 4 November 2026

This policy applies to the Windows version of Moonglow, including its Microsoft Store,
installer, and portable editions. It describes how the app and its maintainers handle
information.

## Information handled by the app

Moonglow adjusts screen brightness and color temperature on your device. It works offline
and does not require an account. The app does not send your settings, display information,
or diagnostic logs to its maintainers or to an external server. It contains no advertising,
tracking, usage analytics, or automatic crash-reporting service.

To provide its features, Moonglow accesses the following information locally:

- Connected display names, identifiers, capabilities, and brightness or color settings,
  so it can detect and control supported displays.
- Your device's local time and Windows appearance setting, so it can apply your schedule
  and match light or dark mode.
- Running process names and process identifiers, solely to check whether another copy of
  Moonglow is already running. This information is not saved as an application-usage history.
- Keyboard shortcuts configured for Moonglow, so its brightness and temperature controls
  work while you use other apps. Moonglow does not record text typed in other applications.

Moonglow does not capture your screen contents or access your browsing history, contacts,
camera, microphone, or geographic location.

## Information stored on your device

Moonglow saves your brightness, color-temperature, schedule, startup, and keyboard-shortcut
preferences in your Windows user profile. Settings are normally stored at
`%APPDATA%\Moonglow\settings.json`; Windows may redirect app-data locations for Microsoft
Store installations. Earlier settings from the app's former name, Amber, may be imported
from `%APPDATA%\Amber\settings.json`.

The app also writes diagnostic logs in the Windows temporary folder, using the filename
`Moonglow-color.log` and numbered backup copies. These logs record timestamps, process and
thread identifiers, requested color temperatures, color-adjustment results, and technical
errors. They help troubleshoot display behavior and are rotated to limit their size.
They are not uploaded automatically.

If you enable launch at sign-in, Moonglow stores its startup registration and executable
path in your Windows user configuration.

These files rely on Windows file permissions for protection; Moonglow does not encrypt
them separately.

## Your choices and deletion

You can change preferences, disable automatic scheduling, clear shortcuts, and turn off
launch at sign-in in the app's settings.

Settings remain on your device until removed. Diagnostic logs remain until rotated,
deleted by you, or cleared by Windows. Uninstalling the app may leave some local files.
To remove remaining app data, first disable launch at sign-in and exit Moonglow, then delete
its settings folder and diagnostic log files, including backup copies. If you previously
used Amber, remove its old settings too if you do not want them imported again.

## Support and third-party services

If you choose to contact the maintainers, they receive the information you submit, such as
your GitHub username, message, and any attachments. This information is used to respond to
your request and investigate or fix reported problems. You choose whether to share logs.

The [Moonglow issue tracker](https://github.com/prompt-smithy/moonglow/issues) is public.
Do not post private information there. Public reports and attachments may remain visible
in the project's history. GitHub handles its platform data under the
[GitHub Privacy Statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).

Windows and Microsoft Store may independently process information for installation,
updates, and system diagnostics under the
[Microsoft Privacy Statement](https://www.microsoft.com/en-us/privacy/privacystatement).
Their data practices are separate from the app's local processing described above.

## Children

The app's local data practices apply to all users. Moonglow does not request personal
information from children through the app.

## Changes and contact

This policy will be updated if Moonglow's data practices change. The date above identifies
the latest revision.

For questions about this policy, contact the Moonglow maintainers through the
[project issue tracker](https://github.com/prompt-smithy/moonglow/issues). For a sensitive
question, request a private contact method without posting the sensitive information.
