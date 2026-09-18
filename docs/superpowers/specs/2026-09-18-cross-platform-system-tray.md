# Cross-platform system tray specification

- Closing the main window must ask whether to exit TunnelDock or minimize it to the system tray.
- The close prompt must also cover operating-system close requests such as Alt+F4, not only the custom title-bar button.
- Choosing exit must preserve the existing asynchronous process cleanup before application shutdown.
- Choosing minimize must hide the main window while keeping the application running.
- The tray context menu must provide “显示主界面” and “退出”.
- Selecting “显示主界面” must show, unminimize, and focus the main window.
- Selecting “退出” must exit through the same cleanup path as a direct close.
- Clicking the tray icon should restore the main window where the platform emits tray click events; Linux must remain fully usable through the context menu.
- The implementation must support Windows, macOS, and Linux without platform-specific application code.

