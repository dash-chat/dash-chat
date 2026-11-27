/** The picture of one scene and what it shows. */
export interface Screenshot {
	/** The name of the scene it is of. */
	name: string;
	/** The picture, as PNG. */
	image: Uint8Array;
	/** The keys of the strings it shows. */
	keys: string[];
}
