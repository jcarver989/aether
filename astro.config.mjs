// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
	integrations: [
		starlight({
			title: 'Aether',
			customCss: ['./src/styles/global.css', './src/styles/starlight.css'],
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/joshka/aether' }],
sidebar: [
				{
					label: 'Getting Started',
					items: [
						{ label: 'Introduction', slug: 'guides/introduction' },
						{ label: 'Quick Start', slug: 'guides/quickstart' },
					],
				},
				{
					label: 'Packages',
					autogenerate: { directory: 'reference' },
				},
			],
		}),
	],
	vite: { plugins: [tailwindcss()] },
});
