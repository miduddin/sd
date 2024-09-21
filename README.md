# sd

A lightweight alternative to Stream Deck app, so lightweight it barely has any
features!

What this app can do:

* Set button image.
* Set the command that will be executed when a button is pressed.

That's it!

In particular, this app has the following limitations:

* No GUI; configure buttons using `buttons.yaml` file.
* Can't set button text.
* No plugin support; only executable commands can be assigned to buttons.
* Only tested on Windows.
* No system tray icon; kill the program from task manager to exit.

## Usage

1. Download the compressed binary file from GitHub release page.
2. Extract the contents to a directory.
3. Configure your buttons in `buttons.yaml` file (see root project directory
   for an example).
4. Run `sd.exe <VID> <PID>`, where `<VID>` and `<PID>` is the hexadecimal
   vendor ID and product ID of your Stream Deck device.
   For example, `sd.exe 0f9d 0080`.

The process should appear in Task Manager if it runs successfully.
