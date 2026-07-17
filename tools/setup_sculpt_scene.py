"""Setup de la escena Blender para esculpir Cortana manualmente.

Ejecutar via BlenderMCP:
    tools/.venv/bin/python -c "
    import sys; sys.path.insert(0, 'tools')
    from ollama_to_blendermcp import BlenderMCPClient
    c = BlenderMCPClient()
    code = open('tools/setup_sculpt_scene.py').read()
    print(c.execute_code(code))
    "
"""

import math

import bpy

# ===== 1. LIMPIAR ESCENA =====
bpy.ops.object.select_all(action="SELECT")
bpy.ops.object.delete()
for block in list(bpy.data.meshes):
    if block.users == 0:
        bpy.data.meshes.remove(block)
for block in list(bpy.data.materials):
    if block.users == 0:
        bpy.data.materials.remove(block)
for block in list(bpy.data.cameras):
    if block.users == 0:
        bpy.data.cameras.remove(block)
for block in list(bpy.data.lights):
    if block.users == 0:
        bpy.data.lights.remove(block)
for block in list(bpy.data.images):
    if block.users == 0:
        bpy.data.images.remove(block)
print("Escena limpia")

# ===== 2. CREAR MATERIAL HOLOGRAFICO AZUL =====
mat = bpy.data.materials.new(name="CortanaHolografico")
mat.use_nodes = True
mat.blend_method = "BLEND"
mat.shadow_method = "NONE"
nodes = mat.node_tree.nodes
links = mat.node_tree.links
for n in list(nodes):
    nodes.remove(n)

output = nodes.new("ShaderNodeOutputMaterial")
output.location = (600, 0)

bsdf = nodes.new("ShaderNodeBsdfPrincipled")
bsdf.location = (200, 100)
bsdf.inputs["Base Color"].default_value = (0.118, 0.565, 0.9, 1.0)
bsdf.inputs["Roughness"].default_value = 0.3
bsdf.inputs["Transmission Weight"].default_value = 0.6
bsdf.inputs["IOR"].default_value = 1.3
bsdf.inputs["Alpha"].default_value = 0.7

emission = nodes.new("ShaderNodeEmission")
emission.location = (200, -100)
emission.inputs["Color"].default_value = (0.118, 0.565, 0.9, 1.0)
emission.inputs["Strength"].default_value = 3.0

mix = nodes.new("ShaderNodeMixShader")
mix.location = (400, 0)
mix.inputs["Fac"].default_value = 0.5

wave = nodes.new("ShaderNodeTexWave")
wave.location = (-100, -200)
wave.inputs["Scale"].default_value = 50.0
wave.wave_type = "BANDS"

ramp = nodes.new("ShaderNodeValToRGB")
ramp.location = (100, -200)
ramp.color_ramp.elements[0].position = 0.4
ramp.color_ramp.elements[1].position = 0.6

links.new(wave.outputs["Fac"], ramp.inputs["Fac"])
links.new(ramp.outputs["Color"], mix.inputs["Fac"])
links.new(bsdf.outputs["BSDF"], mix.inputs[1])
links.new(emission.outputs["Emission"], mix.inputs[2])
links.new(mix.outputs["Shader"], output.inputs["Surface"])

mat.use_backface_culling = False
mat.show_transparent_back = True
print(f"Material creado: {mat.name} (blend={mat.blend_method})")

# ===== 3. CARGAR IMAGEN DE REFERENCIA COMO PLANO DE FONDO =====
# Blender 4.0 removio SpaceView3D.background_images. Usamos un plane
# con la imagen como textura Emission, posicionado detras del modelo.
img_path = "/home/macarthur/Proyectos/SynapseCortana/doc/Cortana.png"
img = bpy.data.images.load(img_path)
img.name = "Cortana_Reference"

# Crear material para el plano de referencia.
ref_mat = bpy.data.materials.new(name="Referencia_Material")
ref_mat.use_nodes = True
ref_nodes = ref_mat.node_tree.nodes
ref_links = ref_mat.node_tree.links
for n in list(ref_nodes):
    ref_nodes.remove(n)
ref_output = ref_nodes.new("ShaderNodeOutputMaterial")
ref_output.location = (400, 0)
ref_tex = ref_nodes.new("ShaderNodeTexImage")
ref_tex.location = (0, 0)
ref_tex.image = img
ref_emission = ref_nodes.new("ShaderNodeEmission")
ref_emission.location = (200, 0)
ref_emission.inputs["Strength"].default_value = 1.0
ref_links.new(ref_tex.outputs["Color"], ref_emission.inputs["Color"])
ref_links.new(ref_emission.outputs["Emission"], ref_output.inputs["Surface"])
ref_mat.blend_method = "BLEND"

# Crear plane vertical detras del modelo (en Y negativo, lejos).
bpy.ops.mesh.primitive_plane_add(
    size=2.5,
    location=(0, 0.5, 1.0),
    rotation=(math.radians(90), 0, 0),
)
ref_plane = bpy.context.active_object
ref_plane.name = "Referencia_Cortana"
ref_plane.data.materials.clear()
ref_plane.data.materials.append(ref_mat)
# El plane mira hacia -Y (hacia la camara). La imagen se ve de frente.
print(f"Plano de referencia creado: {ref_plane.name} con imagen {img.name}")

# Configurar viewport en Material Preview.
for area in bpy.context.screen.areas:
    if area.type == "VIEW_3D":
        for space in area.spaces:
            if space.type == "VIEW_3D":
                space.shading.type = "MATERIAL"
                # Vista frontal: la camara mira hacia +Y (default).
                # No forzar view_rotation; el usuario puede rotar manualmente.
                space.region_3d.view_location = (0, 0, 1.0)
                space.region_3d.view_distance = 5.0
                break
        break

# ===== 4. CREAR CABEZA BASE =====
bpy.ops.mesh.primitive_uv_sphere_add(
    segments=32,
    ring_count=16,
    radius=0.25,
    location=(0, 0, 1.55),
)
head = bpy.context.active_object
head.name = "Cabeza_Base"
bpy.ops.object.shade_smooth()
subsurf = head.modifiers.new(name="Subsurf", type="SUBSURF")
subsurf.levels = 2
subsurf.render_levels = 3
head.data.materials.clear()
head.data.materials.append(bpy.data.materials["CortanaHolografico"])
print(
    f"Cabeza base: {head.name} en {tuple(head.location)} (verts={len(head.data.vertices)})"
)

# ===== 5. CREAR CUERPO BASE =====
bpy.ops.mesh.primitive_cylinder_add(
    vertices=24,
    radius=0.22,
    depth=1.3,
    location=(0, 0, 0.75),
)
body = bpy.context.active_object
body.name = "Cuerpo_Base"
bpy.ops.object.shade_smooth()
subsurf2 = body.modifiers.new(name="Subsurf", type="SUBSURF")
subsurf2.levels = 2
subsurf2.render_levels = 3
body.data.materials.clear()
body.data.materials.append(bpy.data.materials["CortanaHolografico"])
print(
    f"Cuerpo base: {body.name} en {tuple(body.location)} (altura={body.dimensions.z}m)"
)

# ===== 6. CAMARA Y LUCES =====
bpy.ops.object.camera_add(location=(0, -3.5, 1.0))
cam = bpy.context.active_object
cam.name = "Camara_Synapse"
cam.rotation_euler = (math.radians(83), 0, 0)
bpy.context.scene.camera = cam

bpy.ops.object.light_add(type="SUN", location=(2, -2, 5))
sun = bpy.context.active_object
sun.name = "Luz_Sol"
sun.data.energy = 3.0
sun.rotation_euler = (math.radians(45), 0, math.radians(30))

bpy.ops.object.light_add(type="AREA", location=(-2, -1, 2))
fill = bpy.context.active_object
fill.name = "Luz_Relleno_Azul"
fill.data.energy = 50.0
fill.data.color = (0.118, 0.565, 0.9)  # RGB 3-tupla (sin alpha)
fill.data.size = 2.0

print(f"Camara: {cam.name}, Luces: {sun.name} + {fill.name}")
print("\n=== ESCENA LISTA PARA ESCULPIR ===")
print("1. Ve a Blender (ya esta corriendo con la ventana abierta)")
print("2. Veras la imagen de Cortana de fondo al 50% transparencia")
print("3. Selecciona 'Cabeza_Base' o 'Cuerpo_Base' en el Outliner")
print("4. Entra en Sculpt Mode: tecla Ctrl+Tab o boton Mode > Sculpt")
print("5. Usa los pinceles Draw, Grab, Smooth, Inflate para dar forma")
print("6. La imagen de fondo te sirve de guia para proporciones")
print("7. Cuando termines, ejecuta el script de exportacion .glb")
