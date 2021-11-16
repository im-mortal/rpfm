# Contribution

So, you've found yourself wanting to contribute to this project. Or perhaps you’d like to use is as the starting point for your own. This process is known as forking.

Creating a “fork” is producing a personal copy of someone else's project. Forks act as a sort of bridge between the original repository and your personal copy. You can submit pull requests to help make other people's projects better by offering your changes up to the original project. Forking is at the core of social coding at GitHub. For more information, see “[Fork a repo](https://docs.github.com/en/get-started/quickstart/fork-a-repo).”

## Compilation

Just in case someone wants to collaborate with code (who knows, maybe there is someone out there in the wild) here are the **instructions to compile RPFM** in the different supported OS.

[Fork and clone](https://docs.github.com/en/get-started/quickstart/fork-a-repo) the [RPFM repo](https://github.com/Frodo45127/rpfm) with git, then follow the instructions for your operating system.

### Windows

You need to download and install:

- [**Windows SDK**](https://developer.microsoft.com/en-US/windows/downloads/windows-10-sdk).
- [**MSVC**](https://visualstudio.microsoft.com/ru/thank-you-downloading-visual-studio/?sku=community) (with C++ support from the Visual Studio installer).
- [**Rust 1.56 with the MSVC toolchain**](https://www.rust-lang.org/tools/install) (or superior).
- [**Craft**](https://community.kde.org/Guidelines_and_HOWTOs/Build_from_source/Windows) (from KDE).

Once you have Craft installed, you need to install RPFM's dependencies:

```bash
craft -i qtimageformats
craft -i kimageformats
craft -i kwidgetsaddons
craft -i ktexteditor
craft -i kiconthemes
craft -i breeze-icons
```

- If it complains about `libgit2` with an error message mentioning `git_branch_name_is_valid` or something similar, edit the `libgit2` blueprint and make it use `1.2.0`.
  
  You can do that by editing the following file:

  ```plain
  X:/CraftRoot/etc/blueprints/locations/craft-blueprints-kde/libs/libgit2/libgit2.py
  ```

  Change both mentions of `1.1.0` to `1.2.0`. Additionally, either comment out the line starting with
  `self.targetDigests[ver]` or update the SHA256 hash there:

  ```diff
    …
    class subinfo(info.infoclass):
      def setTargets(self):
          self.description = "a portable C library for accessing git repositories"
          self.svnTargets['master'] = 'https://github.com/libgit2/libgit2.git'

          # try to use latest stable libgit2
  -       ver = '1.1.0'
  +       ver = '1.2.0'
          self.targets[ver] = f"https://github.com/libgit2/libgit2/archive/v{ver}.tar.gz"
          self.archiveNames[ver] = f"libgit2-{ver}.tar.gz"
          self.targetInstSrc[ver] = f"libgit2-{ver}"
  -       self.targetDigests[ver] = (['41a6d5d740fd608674c7db8685685f45535323e73e784062cf000a633d420d1e'], CraftHash.HashAlgorithm.SHA256)
  +       self.targetDigests[ver] = (['701a5086a968a46f25e631941b99fc23e4755ca2c56f59371ce1d94b9a0cc643'], CraftHash.HashAlgorithm.SHA256)
          self.defaultTarget = ver
  -       self.patchToApply['1.1.0'] = [("libgit2-pcre2-debugsuffix.diff", 1)]
  +       self.patchToApply['1.2.0'] = [("libgit2-pcre2-debugsuffix.diff", 1)]
          self.patchLevel[self.defaultTarget] = 1
    …
  ```
  
  Then execute:

  ```bash
  craft --set version=1.2.0 libgit2
  craft -i libgit2
  ```

- Then, you also need to edit these two files:
  - `/usr/include/KF5/KTextEditor/ktexteditor/editor.h`
  - `/usr/include/KF5/KTextEditor/ktexteditor/view.h`

  Change the following include in each:

```diff
- #include <KSyntaxHighlighting/Theme>
+ #include <KF5/KSyntaxHighlighting/Theme>
```

Now you can open Craft, move to RPFM source code folder and call from that terminal:

```bash
# To build the executable without optimisations.
cargo build

# To run the ui executable without optimisations (debug mode).
cargo run --bin rpfm_ui

# To build the executable with optimisations (release mode).
cargo build --release
```

You can also make any editor inherit Craft's environment (and thus, being able to compile RPFM) by opening it from Craft's Terminal.

### Linux

You need to install the following packages on your distro:

- **CMake**
- **Rust 1.56** (or superior)
- **Qt 5.14** (or superior)
- **KDE Framework (KF5) 5.61** (or superior)
- **xz**
- **p7zip**

If you use arch or its derivatives, you also need to edit these two files:

- `/usr/include/KF5/KTextEditor/ktexteditor/editor.h`
- `/usr/include/KF5/KTextEditor/ktexteditor/view.h`

  Change the following include in each:

```diff
- #include <KSyntaxHighlighting/Theme>
+ #include <KF5/KSyntaxHighlighting/Theme>
```

Then move to RPFM source code (`cd path/to/rpfm`) and execute the following.

- To build the executable without optimisations:  
  `cargo build`

- To run the ui executable without optimisations (debug mode):  
  `cargo run --bin rpfm_ui`

- To build the executable with optimisations (release mode):  
  `cargo build --release`

### macOS

Don't know. Don't have a Mac to compile to it and test. I tried, it compiles, but its fully untested.

## Contribute to documentation

In case you just want to **contribute to these docs**, you just need to download this repo, install Rust, then move to the repo's folder.

- Install mdBook:  
  `cargo install mdbook`
- Watch your changes to Markdown files updated in live mode:  
  `mdbook watch`
  
  Press <kbd>Ctrl</kbd> <kbd>C</kbd> when done.

- Do a final build and open the resulting mdBook in your browser:  
  `mdbook build --open`  
  Check if the compiled book looks alright.

These commands should work on any OS where you can install Rust on.

Once done, commit your changes to `docs_src` and push them to your RPFM fork.

## Making a Pull Request

A [Pull Request](https://docs.github.com/en/get-started/quickstart/contributing-to-projects#making-a-pull-request) (PR) is the final step in producing a fork of someone else's project, and arguably the most important. If you've made a change to  the source code or the documentation that you feel would benefit the community as a whole, you should definitely consider contributing back.

To do so, head on over to the repository on GitHub where your cloned project lives. In this case, it would be at `https://www.github.com/<your_username>/rpfm`. You'll see a banner indicating that your branch is one commit ahead of `Frodo45127:master`. Click **Contribute** and then **Open a pull request**.

GitHub will bring you to a page where you can enter a title and a description of your changes. It's important to provide as much useful information and a rationale for why you're making this pull request in the first place. The project owner needs to be able to determine whether your change is as useful to everyone as you think it is. Finally, click **Create pull request**.

For more information about [Pull Requests](https://docs.github.com/en/get-started/quickstart/contributing-to-projects#making-a-pull-request) and GitHub, head to [https://docs.github.com](docs.github.com).
