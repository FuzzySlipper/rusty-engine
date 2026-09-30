"""Write render-wgpu's video shader (a Rust string constant) to a .wgsl file.

    extract-video-shader.py <video.rs> <out.wgsl>
"""

import re
import sys

source = open(sys.argv[1]).read()
shader = re.search(r'const SHADER: &str = r#"(.*?)"#;', source, re.S).group(1)
open(sys.argv[2], "w").write(shader)
