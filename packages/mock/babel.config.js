import { signaliumPreset } from 'signalium/transform';

export default {
	presets: ['@babel/preset-typescript', signaliumPreset()],
	targets: {
		browsers: [
			'last 2 Chrome versions',
			'last 2 Safari versions',
			'last 2 iOS versions',
			'last 2 Edge versions',
			'Firefox ESR',
		],
		esmodules: true,
	},
};
