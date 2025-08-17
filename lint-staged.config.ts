import type { Configuration } from 'lint-staged'

const config: Configuration = {
	'*.{js,jsx,ts,tsx}': (stagedFiles) => `oxlint --fix ${stagedFiles.join(' ')}`,
	'*.{js,ts,tsx,yml,yaml,md,json}': (stagedFiles) => `prettier --write ${stagedFiles.join(' ')}`,
	'*.toml': (stagedFiles) => `taplo format ${stagedFiles.join(' ')}`,
	'*.rs': () => ['cargo fmt --all', 'cargo clippy --all-targets --all-features'],
	'*.package.json': () => ['yarn install --mode=update-lockfile'],
}

export default config
