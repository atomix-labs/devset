// Carries a bump from the runner to the repository: one signed commit on the bump branch, then as
// far as `bump.mode` in .github/automation.json says.
//
//   branch  the commit, and an issue with a link to open the pull request
//   pr      also the pull request, opened once and kept up to date; the issue only while it is red
//   merge   also merges it once green: GitHub's auto-merge with an app token, whose pull request
//           runs the required checks; else at once, on the gate this run passed
const fs = require("fs");
const { settings, hold, release } = require(
  `${process.env.GITHUB_WORKSPACE}/.github/scripts/issue.js`,
);

const MODES = ["branch", "pr", "merge"];
const TITLE = "chore(bump): move pinned tools and dependencies";

// Each changed path, and whether it is gone: tracked or not, deletions included.
async function changes(exec) {
  const { stdout } = await exec.getExecOutput("git", [
    "status",
    "--porcelain=v1",
    "-z",
    "--untracked-files=all",
  ], {
    silent: true,
  });
  const entries = stdout.split("\0").filter(Boolean);
  const out = [];
  for (let i = 0; i < entries.length; i++) {
    const code = entries[i].slice(0, 2);
    out.push({ path: entries[i].slice(3), gone: code.includes("D") });
    if (code.startsWith("R")) i++; // a rename's source follows; it reads as a deletion below
  }
  return out;
}

// The body: every report the recipes wrote, and the gate's verdict.
function body(dir, gate, skipped) {
  const reports = fs.existsSync(dir)
    ? fs.readdirSync(dir).sort().map((f) => fs.readFileSync(`${dir}/${f}`, "utf8").trim())
    : [];
  const verdict = gate === "success"
    ? "`just check` passed."
    : "`just check` failed: fix it on the branch.";
  const note = skipped.length
    ? `\n\nLeft out, as this token cannot write workflows: ${
      skipped.map((p) => `\`${p}\``).join(", ")
    }.`
    : "";
  return ["## Weekly bump", ...reports, verdict].join("\n\n") + note;
}

module.exports = async ({ github, context, core, exec }) => {
  const config = settings().bump ?? {};
  const mode = config.mode ?? "pr";
  if (!MODES.includes(mode)) throw new Error(`bump.mode is ${mode}: one of ${MODES.join(", ")}`);
  const branch = config.branch ?? "bot/bump";
  const app = process.env.APP === "true";
  const green = process.env.GATE === "success";
  const base = context.payload.repository.default_branch;
  const head = context.sha;

  const all = await changes(exec);
  const skipped = app
    ? []
    : all.filter((c) => c.path.startsWith(".github/workflows/")).map((c) => c.path);
  const kept = all.filter((c) => !skipped.includes(c.path));
  const ready = "The weekly bump is ready";
  const red = "The weekly bump needs a hand";
  if (!kept.length) {
    core.info("Nothing moved.");
    await release({ github, context }, "chore", ready, "Nothing to bump this week.");
    await release({ github, context }, "broken", red, "Nothing to bump this week.");
    return;
  }

  // The branch starts again from the default branch each week, and takes one signed commit.
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
        message: { headline: TITLE },
        fileChanges: {
          additions: kept.filter((c) => !c.gone).map((c) => ({
            path: c.path,
            contents: read(c.path),
          })),
          deletions: kept.filter((c) => c.gone).map((c) => ({ path: c.path })),
        },
      },
    },
  );
  const text = body(process.env.REPORTS, process.env.GATE, skipped);
  const compare =
    `${context.serverUrl}/${context.repo.owner}/${context.repo.repo}/compare/${base}...${branch}?expand=1`;

  if (mode === "branch") {
    await hold(
      { github, context },
      "chore",
      ready,
      `[Open the pull request](${compare}).\n\n${text}`,
    );
    return;
  }
  await release({ github, context }, "chore", ready, "Now a pull request.");
  const open = await github.rest.pulls.list({
    ...context.repo,
    head: `${context.repo.owner}:${branch}`,
    state: "open",
  });
  const pull = open.data[0]
    ?? (await github.rest.pulls.create({
      ...context.repo,
      head: branch,
      base,
      title: TITLE,
      body: text,
    })).data;
  if (open.data[0]) {
    await github.rest.pulls.update({ ...context.repo, pull_number: pull.number, body: text });
  }
  if (!green) {
    await hold({ github, context }, "broken", red, `The gate failed on ${pull.html_url}.`);
    return;
  }
  await release({ github, context }, "broken", red, `Green again: ${pull.html_url}.`);
  if (mode !== "merge") return;
  if (app) {
    await github.graphql(
      `mutation($id: ID!) { enablePullRequestAutoMerge(input: { pullRequestId: $id, mergeMethod: SQUASH }) { clientMutationId } }`,
      { id: pull.node_id },
    );
  } else {
    await github.rest.pulls.merge({
      ...context.repo,
      pull_number: pull.number,
      merge_method: "squash",
    });
    await github.rest.git.deleteRef({ ...context.repo, ref });
  }
};
