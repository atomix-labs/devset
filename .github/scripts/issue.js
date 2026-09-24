// One issue per condition, found by its exact title: opened when the condition starts, commented on
// while it lasts, closed when it ends. Labels, an issue type and assignees come from
// .github/automation.json, which may leave any of them out.
const fs = require("fs");

const DEFAULTS = {
  broken: { labels: ["bug"] },
  chore: { labels: ["dependencies"] },
  assignees: [],
};

// The automation settings: the repository's own, over the defaults.
function settings() {
  const path = `${process.env.GITHUB_WORKSPACE}/.github/automation.json`;
  const own = fs.existsSync(path) ? JSON.parse(fs.readFileSync(path, "utf8")) : {};
  return { ...DEFAULTS, ...own };
}

// The open issue titled `title` among those labelled as `kind` says, if there is one.
async function find({ github, context }, kind, title) {
  const { labels } = settings()[kind] ?? DEFAULTS[kind];
  const issues = await github.paginate(github.rest.issues.listForRepo, {
    ...context.repo,
    state: "open",
    labels: labels.join(","),
    per_page: 100,
  });
  return issues.find((issue) => issue.title === title && !issue.pull_request);
}

// Opens the issue `title`, or comments on it if it is open: the condition holds.
async function hold({ github, context }, kind, title, body) {
  const open = await find({ github, context }, kind, title);
  if (open) {
    await github.rest.issues.createComment({ ...context.repo, issue_number: open.number, body });
    return open.number;
  }
  const config = settings();
  const { labels, type } = config[kind] ?? DEFAULTS[kind];
  // A raw request, so an issue type goes through where the REST client does not know the field.
  const { data } = await github.request("POST /repos/{owner}/{repo}/issues", {
    ...context.repo,
    title,
    body,
    labels,
    assignees: config.assignees ?? [],
    ...(type ? { type } : {}),
  });
  return data.number;
}

// Comments on the issue `title` and closes it, if it is open: the condition has ended.
async function release({ github, context }, kind, title, body) {
  const open = await find({ github, context }, kind, title);
  if (!open) return;
  await github.rest.issues.createComment({ ...context.repo, issue_number: open.number, body });
  await github.rest.issues.update({ ...context.repo, issue_number: open.number, state: "closed" });
}

module.exports = { settings, find, hold, release };
