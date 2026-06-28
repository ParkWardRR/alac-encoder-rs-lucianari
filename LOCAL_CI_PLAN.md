# Local CI Plan & Agent Instructions

**Attention to any AI Agents or Subagents working in this repository:**

1. **NO GITHUB ACTIONS**: We strictly avoid executing CI on GitHub's cloud infrastructure to avoid billing and quota usage. Do **NOT** create, modify, or expect files in the `.github/workflows/` directory.
2. **Local CI Path**: All CI workflows are maintained in the hidden `.local-ci/workflows/ci.yml` directory.
3. **Execution**: CI is executed locally on macOS using [OrbStack](https://orbstack.dev/) (a lightweight Docker alternative) and `act`. 
4. **Validation Command**: Before finalizing code changes, you must validate them against the CI pipeline by executing:
   ```bash
   act -W .local-ci/workflows
   ```
   Ensure tests pass in this simulated environment.
