// Watches every scheduled workflow, read from .github/workflows/ itself, so none needs listing: one
// whose last scheduled run failed, or that has not run by twice the interval it keeps, or that
// GitHub has disabled, gets an issue until it recovers.
const fs = require("fs");
const { hold, release } = require(`${process.env.GITHUB_WORKSPACE}/.github/scripts/issue.js`);

const FAILED = new Set(["failure", "timed_out", "startup_failure", "action_required"]);
const GRACE = 60 * 60 * 1000;

// The workflow files with a schedule among their triggers.
function scheduled() {
  const dir = `${process.env.GITHUB_WORKSPACE}/.github/workflows`;
  return fs.readdirSync(dir)
    .filter((name) => /\.ya?ml$/.test(name))
    .filter((name) => /^\s+schedule:/m.test(fs.readFileSync(`${dir}/${name}`, "utf8")))
    .map((name) => `.github/workflows/${name}`);
}

// Why the workflow is unwell, or null when it is well, or undefined when there is nothing to say.
async function verdict({ github, context }, workflow) {
  if (workflow.state !== "active") return `is ${workflow.state.replaceAll("_", " ")}`;
  const { data } = await github.rest.actions.listWorkflowRuns({
    ...context.repo,
    workflow_id: workflow.id,
    event: "schedule",
    per_page: 5,
  });
  const runs = data.workflow_runs;
  const done = runs.find((run) => run.status === "completed");
  if (runs.length >= 2) {
    const [last, before] = runs.map((run) => Date.parse(run.created_at));
    if (Date.now() - last > 2 * (last - before) + GRACE) {
      return `has not run on schedule since ${runs[0].created_at}`;
    }
  }
  if (!done) return undefined;
  if (FAILED.has(done.conclusion)) return `failed its scheduled run: ${done.html_url}`;
  return done.conclusion === "success" ? null : undefined;
}

module.exports = async ({ github, context, core }) => {
  const paths = new Set(scheduled());
  const workflows = await github.paginate(github.rest.actions.listRepoWorkflows, {
    ...context.repo,
    per_page: 100,
  });
  for (const workflow of workflows.filter((w) => paths.has(w.path))) {
    const title = `The ${workflow.name} workflow is failing`;
    const why = await verdict({ github, context }, workflow);
    if (why) {
      core.warning(`${workflow.name} ${why}`);
      await hold({ github, context }, "broken", title, `${workflow.name} ${why}.`);
    } else if (why === null) {
      await release({ github, context }, "broken", title, `Recovered at ${context.sha}.`);
    }
  }
};
