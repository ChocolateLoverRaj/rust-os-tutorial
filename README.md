A walkthrough for making your own operating system in Rust, inspired by [Philipp Oppermann's blog "Writing an OS in Rust"](https://os.phil-opp.com/)

## 🚧 In Construction 🚧
Feel free to follow this tutorial. New parts will be added! Keep in mind that there might be changes made to existing parts.

## Contribution Welcome
This is a tutorial and meant to be a community project. Contributions are welcome. Some examples are:
- Fixing typos in tutorial or code
- Making improvements to tutorial or code
- Adding translations
- Making a nice website for this tutorial
- Adding new parts (but make an issue first so we can plan it first)

## Git branches
`main` is where all of the tutorials go (the "production" branch for tutorials). To read the tutorial, look at the `Part ## - */README.md` files (starting with part 0, of course).

`part-#` is where the complete code for the OS at the end of a tutorial part goes (the "production" branches for the complete code). If you're following this tutorial and want to view the OS code for part `#`, do `git worktree add part-# --guess-remote`.

`dev` is the development branch for the tutorials. This is where upcoming tutorials will go before they are merged into `main`. If you're contributing, make your changes (for the tutorial) off of this branch and make merge requests target this branch.

`part-#-dev`  is the development branch for the tutorial OS code. These will eventually get merged into `part-#` branches. Sometimes parts are re-ordered, removed, or inserted between existing parts. When this happens, `part-#-dev` is numbered with the *new*, upcoming part number.

## Git and contributing
### Adding a new part (in the end)
Pretty simple:
- Add a commit to the `main` branch
  - Create `Part ## - <Title>/README.md`, plus you can add additional files such as images. 
- Create a branch `part-#`, based off of the previous part's branch. Here you can make changes to the OS code.

### Changing old parts
If you are changing an old part, or inserting a part before the latest part:
- Update the `main` branch
  - Modify the part that you are changing
  - If you are inserting a part before the last part, renumber all of the next parts
  - If code in the next tutorials changes, update those too
- Update the `part-#` branches
  - Start with the branch you are changing
  - Rebase every branch after that!

Since this is full of rebasing, to save time, talk with me first before making a significant change, even if you are adding a new part (because someone else could also be adding a new part at the same time.)

