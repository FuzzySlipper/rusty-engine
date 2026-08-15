import { mountVoxelSpriteLab } from './voxel-sprite-lab.js';

const root = document.getElementById('voxel-sprite-lab');
if (root === null) throw new Error('missing voxel sprite lab root');
const lab = mountVoxelSpriteLab(root);
window.addEventListener('pagehide', () => lab.dispose(), { once: true });
