# 🎨 akiuS: Aesthetic Spitballing & Creative Directions

This document outlines several diverse, highly contrasting aesthetic directions for **akiuS**. Since the game features sliding, physics-driven spheres that accumulate, merge, and spill off the edge of a table, each theme explores how these physical mechanics translate visually, tactility-wise, and auditorily.

---

## 1. 💾 Retro-Phosphor Terminal (The Baseline Cyberpunk)
*Taking the current terminal vibe to its absolute, high-fidelity limits.*

* **The Vibe:** A diagnostic terminal on a decaying mainframe or a hacking interface inside a retro-futuristic console. Low-level, system-internal, slightly forbidden.
* **Visual Style:**
  * **Table:** A dark-grey terminal screen layout with curved cathode-ray tube (CRT) glass reflections, scanlines, and a slight flicker. The boundaries are glowing terminal brackets.
  * **Spheres:** Wireframe shells enclosing glowing, gaseous plasma cores. As they upgrade, their core brightness increases, changing from dim amber/green to blinding cyan and flashing warnings.
  * **Loss Spill:** Spheres falling off the edge break down into glitching ASCII code cascades (spilling vertical streams of raw data down a terminal error screen).
  * **HUD:** Monospaced terminal font. Score displays as memory addresses or hex dumps (`score = 0x00FF4B`).
* **Tactility & Physics:** Sharp, instant, virtual. Merging looks like a digital burst of pixels or an electric arc snapping between the cores.
* **Audio Landscape:** CRT monitor hum, mechanical relay clicks, digital bleeps, static hiss on merges, and warning alarms when a sphere crosses the Z threshold.

### 🛠️ Implementation in Bevy:
* **Custom CRT Screen Shader (Post-Processing):**
  * Create a custom post-processing WGSL fragment shader that applies to the main 3D camera.
  * Calculate UV distortion using a barrel warp equation to simulate screen curvature: `uv = (uv - 0.5) * (1.0 + barrel_power * dot(uv - 0.5, uv - 0.5)) + 0.5`.
  * Procedurally generate scanlines by modulating brightness based on screen Y coordinate: `color *= 1.0 - scanline_intensity * (0.5 * sin(uv.y * screen_height * 3.1415) + 0.5)`.
  * Add a slight color fringe (chromatic aberration) by shifting red/blue color channel UV lookups.
  * Add a bloom pass on the camera via Bevy's `BloomSettings` to make the glowing vector lines bleed into the dark scanlines.
* **Wireframe Material:**
  * Use Bevy's `StandardMaterial` with `wireframe: true` enabled (requires adding the `bevy::pbr::WireframePlugin` in the App setup).
  * Enclose a smaller glowing sphere inside the wireframe with an emissive material (`emissive: LinearRgba::from(color) * intensity`) to represent the energy core.
* **Glitching Particles:**
  * Spawn a particle system on loss. Particles are 2D text entities (`Text2d`) displaying single random characters from `['0', '1', 'X', '#', '@', '%']` falling downwards while fading out.

### 📦 Asset & Texture Sourcing/Generation:
* **Fonts:** Use free open-source monospace fonts with clean vertical forms.
  * Sourcing: [Google Fonts](https://fonts.google.com/) (*Fira Code*, *JetBrains Mono*, or *Share Tech Mono*).
* **Grid Floor Texture:**
  * Generating: You can build a tileable grid texture programmatically in Rust (using `image` crate) or procedurally in the floor shader.
  * Shader Code Recipe: `let grid = step(0.98, fract(uv.x * 10.0)) + step(0.98, fract(uv.z * 10.0));`.

---

## 2. 🕰️ Gilded Alchemy & Clockwork (Arcane Renaissance)
*Ornate, mechanical craftsmanship combined with mystical occult science.*

* **The Vibe:** A polished mahogany desk in a Victorian observatory, where a mechanical device merges celestial globes. Steampunk, intricate, satisfyingly physical.
* **Visual Style:**
  * **Table:** Dark-polished mahogany wood with gold/brass trim, engraved astrolabes, celestial star maps, and velvet-lined side channels.
  * **Spheres:** Intricately crafted mechanical globes. Tier 1 is copper with exposed tiny cogs. Higher tiers upgrade into brass, polished enamel, marble, ivory, and eventually glass globes housing floating miniature stars or liquid mercury.
  * **Loss Spill:** Spheres rolling off the front clatter off the wooden ledge and plunge into a dark gears-filled abyss below, triggering pneumatic hiss steam effects.
  * **HUD:** Serif calligraphic typeface. Framed in brass filigree.
* **Tactility & Physics:** Heavy, springy, and metallic. When spheres collide, they make clean metal clinks. Merging triggers a satisfying steam release, a gear shift animation, and a spring-winding sound.
* **Audio Landscape:** Clockwork tick-tocks, winding springs, pneumatic hiss releases, chime bells, and heavy wooden thuds.

---

## 3. 🪸 Bioluminescent Abyss (Organic Deep Sea)
*A calm, eerie, organic environment set in the crushing depths of the ocean.*

* **The Vibe:** A hydrothermal vent shelf in the midnight zone of the ocean. Quiet, mysterious, alien, and calming but tense.
* **Visual Style:**
  * **Table:** A dark basalt volcanic rock shelf covered in glowing fungal coral. Deep-sea particles (marine snow) drift slowly through the water column.
  * **Spheres:** Soft, membrane-bound, bioluminescent creatures (egg-like nodes or jellyfish cells) that pulse with internal light. Higher tiers grow internal neural networks, waving micro-tendrils, or swirling glowing spores.
  * **Loss Spill:** Spheres rolling off the edge float/plunge down into the dark open ocean void, losing their light and slowly drifting away as shadows.
  * **HUD:** Fluid, organic UI elements with glowing bioluminescent typography.
* **Tactility & Physics:** Soft, rubbery, jelly-like. Spheres squish slightly on contact before bouncing. Merges feel like fluid droplets fusing together: they snap and meld instantly with a gaseous bubble burst.
* **Audio Landscape:** Low underwater hydrophone rumbles, muffled bubbles, sonar-like pings, liquid squishes, and soft choral synth swells.

### 🛠️ Implementation in Bevy:
* **Custom Bioluminescent Fresnel Shader:**
  * Write a custom WGSL shader for the spheres that simulates a soft outer membrane and glowing interior.
  * Calculate fresnel glow to make the outer edge glow while the center is dark: `let fresnel = pow(1.0 - dot(view_vector, normal), 3.0);`.
  * Animate the glow pulsing over time: `let pulse = 0.5 * sin(time * speed) + 0.5; let final_glow = fresnel * pulse * glow_color;`.
* **Soft-Body Vertex Deformation:**
  * Inside the vertex shader, deform the sphere mesh slightly using simplex noise or simple trigonometric functions over time to make them look like wobbling jellyfish: `position.x += sin(time * 3.0 + position.y) * 0.05;`.
* **Caustics Projection:**
  * Spawn a spot directional light pointing downwards, with its light filter/cookie set to a scrolling caustics texture to simulate water light-refraction.
* **Organic Bubbles Particle System:**
  * Create a particle system that spawns tiny transparent bubbles rising slowly in Y whenever a sphere merges or bounces.

### 📦 Asset & Texture Sourcing/Generation:
* **Bioluminescent Noise Textures:**
  * Generating: Use AI tools like Stable Diffusion or Midjourney to create tiling bio-organic noise patterns.
  * Prompt: `"bioluminescent cells texture, deep sea coral pattern, glowing veins, black background, seamless tiling, PBR map"`
* **Underwater Caustics Map:**
  * Sourcing: Sourced from CC0 texture sites like [Poly Haven](https://polyhaven.com/) or generated using a dedicated tool like *Caustics Generator*.
* **Meshes:**
  * Programmatic: Create standard spheres in Bevy, and deform their vertices inside a custom vertex shader rather than importing high-poly models. This saves disk size and memory.

---

## 4. 🛝 Vaporwave & Liquid Glassmorphism (Retro-Futurism)
*An optimistic, sun-drenched digital playground inspired by early 2000s tech aesthetics.*

* **The Vibe:** A digital beachside resort, translucent plastics, chrome reflections, and nostalgia for the early internet (Y2K / Windows Aero / Dreamcast).
* **Visual Style:**
  * **Table:** A glossy, translucent frosted-glass tray floating over an animated turquoise ocean or a neon grid sunset.
  * **Spheres:** High-gloss liquid mercury blobs, colorful jelly drops, or frosted glass marbles refracting the background light. They feature iridescent finishes, neon gradients (magenta, cyan, peach), and chrome rings.
  * **Loss Spill:** Spheres tumbling off the edge bounce off a rubbery bottom and splash into liquid neon or drift off in slow motion like soap bubbles.
  * **HUD:** Clean sans-serif font (like Outfit or Helvetica) with glassmorphic cards, drop shadows, and neon pink text.
* **Tactility & Physics:** Elastic, bouncy, slick. Collision is clean, like glass marbles clicking. Merging triggers a glossy pop, a wave of pastel ripples on the table surface, and iridescent light rays.
* **Audio Landscape:** Smooth jazz-synth chords, glossy plastic pops, water splashes, and nostalgic retro startup chimes.

---

## 5. 🪨 Brutalist Concrete & Monolithic Stone (Monumental Sculpture)
*An austere, tactile, and heavy stone-cold study in physics and mass.*

* **The Vibe:** A massive architectural sculpture carved out of solid raw concrete and obsidian. Somber, structural, heavy.
* **Visual Style:**
  * **Table:** A slab of raw, porous concrete with deep, cast shadows. The side boundaries are concrete columns.
  * **Spheres:** Heavy, textured stone globes. Tier 1 is rough granite; higher tiers evolve into smooth sandstone, polished marble, layered slate, red clay, dark volcanic basalt, and obsidian carved with geometric runes.
  * **Loss Spill:** Spheres rolling off crash down heavily into a deep concrete trench, fracturing and crumbling into stone dust.
  * **HUD:** Hard, blocky, stencil-cut lettering carved directly into the concrete border.
* **Tactility & Physics:** Incredibly heavy, friction-laden, and grinding. Merging triggers stone cracking, a puff of rock dust, and a heavy grinding shift.
* **Audio Landscape:** Gritty scrapes, stone-on-stone grinding, sharp granite cracking sounds, and deep sub-bass structural rumbles.

### 🛠️ Implementation in Bevy:
* **High-Fidelity PBR Materials:**
  * Use Bevy's built-in `StandardMaterial` with full PBR texture bindings:
    * `base_color_texture`: For stone grain, veins, and concrete pores.
    * `normal_map`: Vital for concrete bumps and stone fractures. Set high `normal_map_scale` to make them feel deeply textured.
    * `perceptual_roughness`: High values (0.8–0.9) for concrete/granite, low values (0.1–0.2) for polished marble and obsidian.
* **Mesh Vertex Displacement:**
  * Instead of perfect spheres, write a shader or process mesh generation to add slight random coordinate noise to the sphere vertices. This makes the rocks look hand-carved and imperfect rather than mathematically perfect.
* **Impact Dust Particle System:**
  * When a collision event triggers with a high impulse (detected via Bevy Rapier's collision data), spawn gray dust particles (`Mesh3d` spheres with flat standard gray material) that expand and fade using alpha transparency.
* **Engraved Emissive Runes:**
  * For higher tiers (like obsidian), use an emissive texture map to draw glowing runes onto the stone surface, suggesting magical containment.

### 📦 Asset & Texture Sourcing/Generation:
* **PBR Stone & Concrete Textures:**
  * Sourcing: Free CC0 PBR textures can be downloaded from [AmbientCG](https://ambientcg.com/) or [Poly Haven](https://polyhaven.com/) (look for *Concrete*, *Slate*, *Granite*, and *Marble* categories).
* **AI Texture Prompts:**
  * Prompt: `"raw porous brutalist concrete slab, flat view, high detail, seamless tiling, slate gray, PBR textures"`
  * Normal Map Generation: Convert AI flat image outputs to normal maps using free tools like [normalmap.online](https://cpetry.github.io/NormalMap-Online/) or *Materialize*.

---

## 6. 🎋 Zen Rock Garden (Sumi-e / Calligraphy)
*A quiet, meditative sandbox focused on balance, minimalism, and flowing strokes.*

* **The Vibe:** A traditional Japanese sand garden. Peaceful, natural, ink-brushed, and flowing.
* **Visual Style:**
  * **Table:** A box of raked white sand with concentric circular patterns. The edges are dark cedar wood.
  * **Spheres:** Smooth, weathered river stones (some grey, some black, some white). Instead of color gradients, they feature elegant glowing brush-stroke calligraphy symbols (calligraphy numbers or Kanji representing elements like *Wood, Fire, Water, Earth, Metal*).
  * **Loss Spill:** Stones roll off the wooden frame and slide silently into a bed of moss or dry leaves.
  * **HUD:** Brush-stroke fonts overlaying a soft paper-textured parchment card.
* **Tactility & Physics:** Smooth, sliding, quiet. Minimal bounciness. Merges trigger a soft ink-bleed visual effect where the symbols swirl together like ink dropping into water.
* **Audio Landscape:** Raked sand sweeps, wind through bamboo, a wooden shishi-odoshi (bamboo water fountain) clack, and quiet flute/koto strums.

---

## 7. 🌌 Cosmic Singularity (Quantum Gravity)
*Playing with astrophysics, celestial bodies, and gravitational warp.*

* **The Vibe:** An experimental simulation of space-time fabric at the edge of the universe. Vast, awe-inspiring, and scientific.
* **Visual Style:**
  * **Table:** A coordinate grid representing a warped space-time membrane, bending dynamically under the weight of active spheres. The background is a swirling nebula.
  * **Spheres:** Miniature celestial bodies. Tier 1 is a tiny asteroid; higher tiers become moon-like rock cores, blue gas giants, protostars, glowing neutron stars, and eventually a singularity (black hole) with a gravitational lensing shader.
  * **Loss Spill:** Spheres rolling off the front get pulled into the event horizon of a massive black hole below, stretching visually (spaghettification) as they fall.
  * **HUD:** Sci-fi telemetry readout style (vector lines, scan coordinates, orbital math).
* **Tactility & Physics:** Float-heavy, pulling towards merges. Collisions cause gravitational distortion waves across the grid.
* **Audio Landscape:** Deep pulsar sweeps, sub-bass grav-vibrations, space hums, and cosmic winds.

### 🛠️ Implementation in Bevy:
* **Dynamic Grid Deformation (Vertex Shader):**
  * Create a high-density flat mesh grid for the board floor.
  * Bind a structured buffer containing the world positions and masses of all active spheres to the floor grid shader.
  * In the floor's vertex shader, offset the Y position of the grid vertices downward based on the proximity of the spheres:
    * `displacement.y = sum_i( -gravity_strength * mass_i / (distance_i + 0.1) );`.
    * This creates physical "wells" in the grid under each sphere that update in real time.
* **Gravitational Lensing screen shader:**
  * Implement a full-screen shader pass.
  * Pass the screen-space coordinates of the highest-tier sphere (black hole) to the shader.
  * Distort the texture lookup coordinates near the black hole center to simulate gravitational light warping: `distorted_uv = blackhole_center + normalize(uv - blackhole_center) * pow(length(uv - blackhole_center), lens_power);`.
* **Volumetric Nebulae (Shaders):**
  * Render gas giants and stars using a sphere mesh with a raymarching shader that calculates noise-based density inside the sphere boundary, creating gas bands or solar flares.

### 📦 Asset & Texture Sourcing/Generation:
* **HDR Space Skybox:**
  * Sourcing: Sourced from [Poly Haven](https://polyhaven.com/) (they have a few starfield/cosmic HDRIs) or space assets packs on [OpenGameArt](https://opengameart.org/).
  * AI Skybox Generation: Use skybox generator services like *Blockade Labs Skybox AI* or generate panoramic space nebulae via Midjourney.
  * Prompt: `"equirectangular 360 panorama of dark space, violet and gold cosmic nebula, distant starfield, high resolution, HDR"`
* **Asteroid/Moon Textures:**
  * Sourcing: Use Mars/Moon surface textures or crater normal maps from NASA's public repositories or CC0 textures.
