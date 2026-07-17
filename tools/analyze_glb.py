"""Analiza el .glb del usuario: vertices, triangulos, materiales, camaras, armatures."""

import bpy

# Limpiar escena.
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
for block in list(bpy.data.actions):
    bpy.data.actions.remove(block)

# Importar el .glb del usuario.
glb_path = "/home/macarthur/Proyectos/SynapseCortana/frontend/assets/Cortana.glb"
bpy.ops.import_scene.gltf(filepath=glb_path)

# Listar objetos importados.
imported = [o for o in bpy.data.objects if o.type == "MESH"]
print("Objetos importados:", len(imported))
total_verts = 0
total_tris = 0
for obj in imported:
    verts = len(obj.data.vertices)
    tris = sum(len(p.vertices) - 2 for p in obj.data.polygons)
    total_verts += verts
    total_tris += tris
    mats = [m.name for m in obj.data.materials if m]
    dims = tuple(round(d, 3) for d in obj.dimensions)
    loc = tuple(round(v, 3) for v in obj.location)
    print("  - {}: {} verts, ~{} tris, mats={}, dims={}, loc={}".format(
        obj.name, verts, tris, mats, dims, loc
    ))
print("\nTOTAL: {} vertices, ~{} triangulos".format(total_verts, total_tris))

# Listar todos los materiales.
print("\nMateriales en la escena:")
for mat in bpy.data.materials:
    print("  - {} (blend={}, use_nodes={})".format(
        mat.name, mat.blend_method, mat.use_nodes
    ))

# Camaras, luces, armatures.
cams = [o for o in bpy.data.objects if o.type == "CAMERA"]
lights = [o for o in bpy.data.objects if o.type == "LIGHT"]
armatures = [o for o in bpy.data.objects if o.type == "ARMATURE"]
print("\nCamaras: {}, Luces: {}, Armatures: {}".format(
    len(cams), len(lights), len(armatures)
))

# Animaciones.
print("Animaciones (actions):", len(bpy.data.actions))
for act in bpy.data.actions:
    print("  -", act.name, "frames:", act.frame_range)

# Bounding box global.
if imported:
    import mathutils
    all_corners = []
    for obj in imported:
        for corner in obj.bound_box:
            all_corners.append(obj.matrix_world @ mathutils.Vector(corner))
    if all_corners:
        min_x = min(c.x for c in all_corners)
        max_x = max(c.x for c in all_corners)
        min_y = min(c.y for c in all_corners)
        max_y = max(c.y for c in all_corners)
        min_z = min(c.z for c in all_corners)
        max_z = max(c.z for c in all_corners)
        print("\nBounding box global:")
        print("  X: {} to {} (width={})".format(round(min_x, 3), round(max_x, 3), round(max_x - min_x, 3)))
        print("  Y: {} to {} (depth={})".format(round(min_y, 3), round(max_y, 3), round(max_y - min_y, 3)))
        print("  Z: {} to {} (height={})".format(round(min_z, 3), round(max_z, 3), round(max_z - min_z, 3)))
