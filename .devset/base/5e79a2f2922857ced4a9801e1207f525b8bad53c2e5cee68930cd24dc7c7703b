// Carries the recordings the demo workflow made to the repository: one signed commit on a branch
// of its own, named for the release, and a pull request, labelled and assigned, for a person to
// merge; with `demo.merge` in .github/automation.json, which the vhs feature `merge` sets, it
// merges itself, every tape having played to its end. Nothing, when every recording is as it was.
const fs = require("fs");
const { settings, propose, HELD, merges, message, land } = require(
  `${process.env.GITHUB_WORKSPACE}/.github/scripts/automation.js`,
);

const MEDIA = "docs/src/media";

// Each recording that changed, and whether it is gone.
async function changes(exec) {
  const { stdout } = await exec.getExecOutput("git", [
    "status",
    "--porcelain=v1",
    "-z",
    "--untracked-files=all",
    "--",
    MEDIA,
  ], {
    silent: true,
  });
  return stdout.split("\0").filter(Boolean).map((entry) => ({
    path: entry.slice(3),
    gone: entry.slice(0, 2).includes("D"),
  }));
}

module.exports = async ({ github, context, core, exec }) => {
  const app = process.env.APP === "true";
  const wanted = settings().demo?.merge === true;
  const merging = await merges({ github, context, core }, wanted, app);
  const changed = await changes(exec);
  if (!changed.length) {
    core.info("Every recording is as it was.");
    return;
  }
  const tag = context.ref.startsWith("refs/tags/") ? context.ref.slice("refs/tags/".length) : null;
  const branch = `demo/${tag ?? context.runId}`;
  const title = tag ? `docs: record the demo for ${tag}` : "docs: record the demo";
  const head = context.sha;
  // A run again for the same release starts the branch again.
  const ref = `heads/${branch}`;
  try {
    await github.rest.git.updateRef({ ...context.repo, ref, sha: head, force: true });
  } catch {
    await github.rest.git.createRef({ ...context.repo, ref: `refs/${ref}`, sha: head });
  }
  const read = (path) =>
    fs.readFileSync(`${process.env.GITHUB_WORKSPACE}/${path}`).toString("base64");
  await github.graphql(
    `mutation($input: CreateCommitOnBranchInput!) { createCommitOnBranch(input: $input) { commit { oid } } }`,
    {
      input: {
        branch: {
          repositoryNameWithOwner: `${context.repo.owner}/${context.repo.repo}`,
          branchName: branch,
        },
        expectedHeadOid: head,
        message: message(title, { merging, app }),
        fileChanges: {
          additions: changed.filter((c) => !c.gone).map((c) => ({
            path: c.path,
            contents: read(c.path),
          })),
          deletions: changed.filter((c) => c.gone).map((c) => ({ path: c.path })),
        },
      },
    },
  );
  const recorded = changed.map((c) => `\`${c.path}\``).join(", ");
  const body = `Recorded by the demo workflow from \`docs/demo/*.tape\`${
    tag ? ` at ${tag}` : ""
  }: ${recorded}.`;
  const again = `The demo was recorded again${tag ? ` for ${tag}` : ""}, every tape to its end.`;
  const note = merging ? undefined : `${again} ${wanted ? HELD : "Look at it, and merge."}`;
  const pull = await propose({ github, context }, "docs", { branch, title, body, note });
  if (merging) {
    await land({ github, context }, pull, { app, branch, said: again });
  }
  const done = !merging ? "Opened" : app ? "Opened, to merge once its checks pass," : "Merged";
  core.notice(`${done} ${pull.html_url}`);
};
