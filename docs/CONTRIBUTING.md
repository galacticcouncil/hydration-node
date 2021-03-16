## Contributing

The `HydraDX` project is an Open Source Project if you'd like to help us please follow these rules so that our lives are easy

 - [Code of Conduct](#coc)
 - [Question or Problem?](#question)
 - [Issues and Bugs](#issue)
 - [Feature Requests](#feature)
 - [Submission Guidelines](#submit)
 - [Coding Rules](#rules)
 - [Pull Request Naming Guidelines](#conventional)
 - [SemVer](#semver)

## <a name="coc"></a> Code of Conduct
Help us keep HydraDX open and inclusive. Please read and follow our [coc](CODE_OF_CONDUCT.md).

## <a name="question"></a> Got a Question or Problem?

Do not open issues for general support questions as we want to keep GitHub issues for bug reports and feature requests. You've got much better chances of getting your question answered on [Discord](https://discord.gg/T8HfDP6d83).

## <a name="issue"></a> Found a Bug?
If you find a bug in the source code, you can help us by submitting an issue to our repository. 
Even better, you can submit a PR with a fix.

## <a name="feature"></a> Missing a Feature?
You can *request* a new feature by submitting an issue to our GitHub
Repository. 
If you would like to *implement* a new feature, please submit an issue with
a proposal for your work first, to be sure that we can use it.
Please consider what kind of change it is:

* For a **Major Feature**, first open an issue and outline your proposal so that it can be
discussed. This will also allow us to better coordinate our efforts, prevent duplication of work,
and help you to craft the change so that it is successfully accepted into the project.
* **Small Features** can be crafted and directly [submitted as a Pull Request](#submit-pr).

## <a name="submit"></a> Submission Guidelines

### <a name="submit-issue"></a> Submitting an Issue

Before you submit an Issue, please search Issues for an open or closed Issue in this repository.
Please use the provided templates to create new issues.

### <a name="submit-pr"></a> Submitting a Pull Request (PR)
Before you submit your Pull Request (PR) consider the following guidelines:

1. Search GitHub for an open or closed PR that relates to your submission. You don't want to duplicate effort.
1. Please sign our [Contributor License Agreement (CLA)](#cla) before sending PRs. We cannot accept code without this. Make sure you sign with the primary email address.
1. Fork the repository.
1. Make your changes in a new git branch:

     ```shell
     git checkout -b fix/my-branch master
     ```

1. Create your patch, **including appropriate test cases**.
1. Follow our [Coding Rules](#rules).
1. Run the respective test suites and make sure it passes.
1. Use respective formatting tools to format the code.
1. Commit your changes using a descriptive commit message.

     ```shell
     git commit -a
     ```
    Note: the optional commit `-a` command line option will automatically "add" and "rm" edited files.

1. Push your branch to GitHub:

    ```shell
    git push origin fix/my-branch
    ```

1. In GitHub, send a pull request to `master` or `main` branch.
1. Make sure the PR name follows [conventional commit format](#conventional)
1. Make sure you bumped the correct versions. We're following [SemVer](#semver)
* If we suggest changes then:
  * Make the required updates.
  * Re-run the test suites to ensure tests are still passing.

That's it! Thank you for your contribution!

#### After your pull request is merged

After your pull request is merged, you can safely delete your branch and pull the changes
from the main (upstream) repository:

* Delete the remote branch on GitHub either through the GitHub web UI or your local shell as follows:

    ```shell
    git push origin --delete fix/my-branch
    ```

* Check out the master branch:

    ```shell
    git checkout master -f
    ```

* Delete the local branch:

    ```shell
    git branch -D fix/my-branch
    ```

* Update your master with the latest upstream version:

    ```shell
    git pull --ff upstream master
    ```

Individuals making significant and valuable contributions are given
commit-access to the project to contribute as they see fit.

## <a name="rules"></a> Coding Rules
To ensure consistency throughout the source code, keep these rules in mind as you are working:

* All features or bug fixes **must be tested** by one or more specs unit tests.
* All module API methods **must be documented**.
* Please follow our [style guide](STYLE_GUIDE.md)

## <a name="conventional"></a> Pull Request Naming Guidelines

We have very precise rules over how our git pull request names can be formatted.  This leads to **more
readable messages** that are easy to follow when looking through the **project history**.  But also,
we use the git merge commit messages to **generate the change logs**.

### The format
Each pull-request name consists of a **header**, a **body** and a **footer**.  The header has a special
format that includes a **type**, a **scope** and a **subject**:

```
<type>(<scope>)<breaking-change-indicator>: <subject>
```

Samples:

```
ci(changelog): generate changelog
```
```
fix(amm): fix benchmarking
```
```
feat(claim)!: add possibility to change claim data
```
* Note that last message indicates a breaking change

### Type
Must be one of the following:

* **build**: Changes that affect the build system or external dependencies (example scopes: cargo, substrate)
* **ci**: Changes to our CI configuration files and scripts (example scopes: gh-actions, bench-bot)
* **docs**: Documentation only changes
* **feat**: A new feature
* **fix**: A bug fix (example scopes: amm, claims)
* **perf**: A code change that improves performance
* **refactor**: A code change that neither fixes a bug nor adds a feature
* **style**: Changes that do not affect the meaning of the code (white-space, formatting, missing semi-colons, etc)
* **test**: Adding missing tests or correcting existing tests (example scopes: amm, claims)

### Scope
The scope should be the name of the affected module (as perceived by the person reading the changelog generated from merge commit messages).
If the PR touches more than one thing, leave it blank and try to use subject to describe the change.

### Subject
The subject contains a succinct description of the change:

* use the imperative, present tense: "change" not "changed" nor "changes"
* don't capitalize the first letter
* no dot (.) at the end

**Breaking Changes** add ! after the scope

## <a name="semver"></a> SemVer
We're following [SemVer](https://semver.org) specification to keep us and our dependencies sane.

* please bump respective version of the closest package file (cargo.toml or package.json).
* if the change is a breaking change, always change the MAJOR version and bump version of packages above the package it's using. Follow this guideline to the root of the repository.
* if in the Node repository, bump impl_version (non-breaking change) and spec_version (breaking-change) in the `runtime/lib.rs` respectively.

### We're thankful for any meaningful contribution

Feedback for this guide is welcome! If you believe
you have something valuable to add or change, please open a PR.

These contributing guidelines are modified from the [Angular](https://raw.githubusercontent.com/angular/angular/22b96b96902e1a42ee8c5e807720424abad3082a/CONTRIBUTING.md) guidelines.
