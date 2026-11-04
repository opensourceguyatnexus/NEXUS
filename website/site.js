// The hero mark: the NEXUS logo built in 3D. It follows the pointer and renders only
// while on screen. The flat logo stays in place when motion is reduced, on small
// screens, without WebGL, or if anything here fails to load.

const host = document.getElementById("chip");
const skip =
  matchMedia("(prefers-reduced-motion: reduce)").matches ||
  matchMedia("(max-width: 900px)").matches ||
  !hasWebGL();

function hasWebGL() {
  try {
    return Boolean(document.createElement("canvas").getContext("webgl2"));
  } catch {
    return false;
  }
}

if (!skip) {
  // Load 3D after the page is usable, so it never delays the content.
  addEventListener("load", () => start().catch(() => host.classList.remove("live")), { once: true });
}

async function start() {
  const THREE = await import("three");
  const { RoomEnvironment } = await import("./vendor/RoomEnvironment.js");

  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  host.append(renderer.domElement);

  const scene = new THREE.Scene();
  const pmrem = new THREE.PMREMGenerator(renderer);
  scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
  scene.environmentIntensity = 0.55;

  const camera = new THREE.PerspectiveCamera(30, 1, 0.1, 50);
  camera.position.set(0, 0, 9.2);

  const key = new THREE.DirectionalLight(0xffffff, 0.5);
  key.position.set(-3, 4, 6);
  // A blue light from one side ties the mark to the accent color.
  const rim = new THREE.DirectionalLight(0x3c9be8, 2.4);
  rim.position.set(5, -2, 2);
  scene.add(key, rim);

  // The logo's own outline, from its 256-unit drawing grid.
  const outline = [
    [24, 68], [68, 24], [88, 24], [172, 150], [172, 24], [232, 24],
    [232, 232], [172, 232], [84, 106], [84, 232], [24, 232],
  ].map(([x, y]) => [(x - 128) / 76, (128 - y) / 76]);
  const shape = new THREE.Shape();
  outline.forEach(([x, y], i) => (i ? shape.lineTo(x, y) : shape.moveTo(x, y)));
  shape.closePath();

  const geometry = new THREE.ExtrudeGeometry(shape, {
    depth: 0.5,
    bevelEnabled: true,
    bevelSize: 0.035,
    bevelThickness: 0.035,
    bevelSegments: 3,
  });
  geometry.center();
  const mark = new THREE.Mesh(
    geometry,
    new THREE.MeshStandardMaterial({ color: 0xbdbdbd, metalness: 1, roughness: 0.34 }),
  );
  scene.add(mark);

  const resize = () => {
    const s = renderer.domElement.parentElement.clientWidth * 1.24;
    renderer.setSize(s, s, false);
  };
  resize();
  addEventListener("resize", resize);

  let aimX = 0;
  let aimY = 0;
  addEventListener(
    "pointermove",
    (e) => {
      aimX = (e.clientX / innerWidth - 0.5) * 2;
      aimY = (e.clientY / innerHeight - 0.5) * 2;
    },
    { passive: true },
  );

  let onScreen = true;
  new IntersectionObserver(([entry]) => {
    onScreen = entry.isIntersecting;
  }).observe(host);

  // Starts turned away and settles facing you: the page's one orchestrated moment.
  const started = performance.now();
  renderer.setAnimationLoop((now) => {
    if (!onScreen || document.hidden) return;
    const t = Math.min(1, (now - started) / 1400);
    const settle = (1 - t) ** 3;
    const sway = Math.sin(now / 2600) * 0.05;
    mark.rotation.y += (aimX * 0.42 + sway - 0.3 + settle * 1.5 - mark.rotation.y) * 0.08;
    mark.rotation.x += (aimY * 0.32 - 0.12 - mark.rotation.x) * 0.08;
    renderer.render(scene, camera);
  });

  host.classList.add("live");
}
