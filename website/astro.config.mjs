// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import cloudflare from '@astrojs/cloudflare';

// https://astro.build/config
export default defineConfig({
	output: 'static',
	adapter: cloudflare({
		mode: 'directory'
	}),
	integrations: [
		starlight({
			title: 'cuenv',
			description: 'A direnv alternative that uses CUE files for environment configuration',
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/rawkode/cuenv' }],
			customCss: ['./src/styles/global.css'],
			expressiveCode: {
				// Modern window decorations configured via CSS
				defaultProps: {
					// Terminal frame for shell commands
					overridesByLang: {
						bash: { frame: 'terminal' },
						sh: { frame: 'terminal' },
						zsh: { frame: 'terminal' },
						fish: { frame: 'terminal' },
					},
				},
			},
			sidebar: [
				{
					label: 'Getting Started',
					items: [
						{ label: 'Introduction', slug: 'intro' },
						{ label: 'Installation', slug: 'installation' },
						{ label: 'Quick Start', slug: 'quickstart' },
					],
				},
				{
					label: 'Guides',
					items: [
						{ label: 'CUE File Format', slug: 'guides/cue-format' },
						{ label: 'Secret Management', slug: 'guides/secrets' },
						{ label: 'Environments', slug: 'guides/environments' },
						{ label: 'Capabilities', slug: 'guides/capabilities' },
						{ label: 'Shell Integration', slug: 'guides/shell-integration' },
					],
				},
				{
					label: 'Reference',
					items: [
						{ label: 'Commands', slug: 'reference/commands' },
						{ label: 'Configuration', slug: 'reference/configuration' },
						{ label: 'Environment Variables', slug: 'reference/env-vars' },
					],
				},
			],
		}),
	],
});
