// Carries the recordings the demo workflow made to a pull request: one signed commit on a branch of
// its own, named for the release; nothing when every recording is as it was.
const fs = require("fs");

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
        message: { headline: title },
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
  const open = await github.rest.pulls.list({
    ...context.repo,
    head: `${context.repo.owner}:${branch}`,
    state: "open",
  });
  if (open.data.length) {
    core.notice(`Updated ${open.data[0].html_url}`);
    return;
  }
  const { data } = await github.rest.pulls.create({
    ...context.repo,
    head: branch,
    base: context.payload.repository.default_branch,
    title,
    body: "Recorded by the demo workflow from `docs/demo/*.tape`.",
  });
  core.notice(`Opened ${data.html_url}`);
};
