A walkthrough for making your own operating system in Rust, inspired by [Philipp Oppermann's blog "Writing an OS in Rust"](https://os.phil-opp.com/).

## Version
You are viewing `v2`.

## 🚧 In Construction 🚧
Feel free to follow this tutorial. New parts will be added! New parts might also come as a new tutorial version, in which case you might have to make some adjustments to your OS first before following the tutorial for new parts.

## Contribution Welcome
This is a tutorial and meant to be a community project. Contributions are welcome. Some examples are:
- Fixing typos in tutorial or code
- Making improvements to tutorial or code
- Adding translations
- Making a nice website for this tutorial
- Adding new parts (but make an issue first so we can plan it first)

## Git branches
`main` is where all of the tutorials go (the "production" branch for tutorials). The tutorial uses [mdBook](https://github.com/rust-lang/mdBook). You can read about its [installation](https://rust-lang.github.io/mdBook/guide/installation.html) and the [commands to generate the book](https://rust-lang.github.io/mdBook/cli/index.html). `dev` is similar to `main`, but it is a development / preview branch.

`code` is the branch which contains the code that goes along with the tutorial. Each commit is 1 part of the tutorial. To edit `main` and `code` at the same time, you can use `git worktree add code --guess-remote`, and the `code` branch will be in the `code` folder.
ming part number. `code-dev` is the corresponding development branch.

`next` is an OS that is very similar to the tutorial OS where I try out implementing new things. It is kind of a preview of what will come to this tutorial. It is kind of messy because I might rebase `next` based on the tutorial, but right now it's based on an old tutorial version.

## Git and contributing
### Adding a new part (in the end)
Pretty simple:
- Add a commit to the `main` branch
  - Create `Part ## - <Title>/README.md`, plus you can add additional files such as images. 
- Create a commit on `code`. Here you can make changes to the OS code.

### Changing existing parts
If you are changing existing old part, or inserting a part before the latest part:
- Update the `main` branch
  - Modify the part that you are changing
  - If you are inserting a part before the last part, renumber all of the next parts
  - If code in the next tutorials changes, update those too
- Update the `code` branch
  - Use `git rebase` to edit the commit for the part you are modifying

Since this is full of rebasing, to save time, talk with me first before making a significant change, even if you are adding a new part (because someone else could also be adding a new part at the same time.)

## Viewing the old version
If you started viewing the `v1` version and want to continue it, you can view the `v1` git tag. You can view the parts at `v1p<n>` where `n` is the part number. Still, I would recommend starting over from `v2`.
