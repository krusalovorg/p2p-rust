// graph-hero.js

// Получаем контейнер и размеры
const container = document.getElementById('graph-canvas-container');
const canvas = document.getElementById('graph-canvas');

// Размеры canvas подстраиваем под контейнер
function resizeCanvas() {
    canvas.width = container.offsetWidth;
    canvas.height = container.offsetHeight;
}
resizeCanvas();
window.addEventListener('resize', resizeCanvas);

// Three.js сцена
const scene = new THREE.Scene();
const camera = new THREE.PerspectiveCamera(60, canvas.width / canvas.height, 0.1, 1000);
const renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true });
renderer.setSize(canvas.width, canvas.height);

// --- Glassmorphism Material ---
function createGlassMaterial(color) {
    return new THREE.MeshPhysicalMaterial({
        color: color,
        metalness: 0.1,
        roughness: 0.15,
        transmission: 0.85, // стеклянность
        thickness: 0.5,
        transparent: true,
        opacity: 0.32,
        ior: 1.4,
        clearcoat: 1.0,
        clearcoatRoughness: 0.1,
        reflectivity: 0.18,
    });
}

// --- Nodes ---
const peerCount = 5;
const radius = 0.55;
const centerNode = new THREE.Mesh(
    new THREE.SphereGeometry(0.09, 32, 32),
    createGlassMaterial(0x99eaff)
);
scene.add(centerNode);

const peerNodes = [];
for (let i = 0; i < peerCount; i++) {
    const mesh = new THREE.Mesh(
        new THREE.SphereGeometry(0.06, 32, 32),
        createGlassMaterial(0xffffff)
    );
    scene.add(mesh);
    peerNodes.push(mesh);
}

// --- Lines ---
const lines = [];
for (let i = 0; i < peerCount; i++) {
    const geometry = new THREE.BufferGeometry().setFromPoints([
        new THREE.Vector3(0, 0, 0),
        new THREE.Vector3(0, 0, 0)
    ]);
    const material = new THREE.LineBasicMaterial({ color: 0x99eaff, transparent: true, opacity: 0.13 });
    const line = new THREE.Line(geometry, material);
    scene.add(line);
    lines.push(line);
}

// --- Bloom Effect (glow) ---
const composer = new THREE.EffectComposer(renderer);
const renderPass = new THREE.RenderPass(scene, camera);
composer.addPass(renderPass);
const bloomPass = new THREE.UnrealBloomPass(
    new THREE.Vector2(canvas.width, canvas.height),
    0.45, // strength
    0.8, // radius
    0.15 // threshold
);
composer.addPass(bloomPass);

// Камера
camera.position.z = 1.1;

// --- Реакция на мышку ---
let mouse = { x: 0, y: 0 };
container.addEventListener('mousemove', (e) => {
    const rect = container.getBoundingClientRect();
    // координаты мыши в [-1, 1]
    mouse.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
    mouse.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;
});

// Анимация
let t = 0;
function animate() {
    requestAnimationFrame(animate);
    t += 0.012;
    // Центр всегда в центре
    centerNode.position.set(0, 0, 0);
    // Пиры по кругу, но слегка отталкиваются от мыши
    for (let i = 0; i < peerCount; i++) {
        let baseAngle = (i / peerCount) * Math.PI * 2 + t * 0.08;
        // Смещение от мыши
        let mx = mouse.x * 0.5;
        let my = mouse.y * 0.5;
        let angle = baseAngle + (mx * Math.sin(baseAngle) + my * Math.cos(baseAngle)) * 0.25;
        let r = radius + Math.sin(t + i) * 0.03;
        let x = Math.cos(angle) * r;
        let y = Math.sin(angle) * r;
        peerNodes[i].position.set(x, y, 0);
        // Линия
        const points = [
            new THREE.Vector3(0, 0, 0),
            peerNodes[i].position.clone()
        ];
        lines[i].geometry.setFromPoints(points);
    }
    // Resize
    renderer.setSize(container.offsetWidth, container.offsetHeight, false);
    composer.setSize(container.offsetWidth, container.offsetHeight);
    camera.aspect = container.offsetWidth / container.offsetHeight;
    camera.updateProjectionMatrix();
    composer.render();
}
animate();

// --- CSS blur for canvas (доп. эффект стекла) ---
canvas.style.filter = 'blur(2.5px)';
canvas.style.opacity = '0.95'; 