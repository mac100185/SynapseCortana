"""Exporta el modelo esculpido a .glb después de esculpir manualmente.

Uso (después de esculpir en Blender):
    tools/.venv/bin/python -c "
    import sys; sys.path.insert(0, 'tools')
    from ollama_to_blendermcp import BlenderMCPClient
    c = BlenderMCPClient()
    code = open('tools/export_sculpted_model.py').read()
    print(c.execute_code(code))
    "

Qué hace:
1. Vuelve a Object Mode (por si estás en Sculpt Mode).
2. Selecciona todas las mallas (excepto el plano de referencia).
3. Las une en un solo objeto "Cortana_Holografica".
4. Aplica el modificador Subdivision Surface.
5. Aplica las transformadas.
6. Exporta a .glb en frontend/assets/cortana_completa.glb.
"""

import bpy

# ===== 1. VOLVER A OBJECT MODE =====
if bpy.context.object and bpy.context.object.mode != "OBJECT":
    bpy.ops.object.mode_set(mode="OBJECT")
print("Modo Object activo")

# ===== 2. SELECCIONAR SOLO MALLAS (NO referencias ni luces ni camaras) =====
bpy.ops.object.select_all(action="DESELECT")
mesh_objects = []
for obj in bpy.data.objects:
    if obj.type == "MESH" and obj.name != "Referencia_Cortana":
        obj.select_set(True)
        mesh_objects.append(obj)

if not mesh_objects:
    print("ERROR: no hay mallas para exportar")
else:
    # Activar la primera malla.
    bpy.context.view_layer.objects.active = mesh_objects[0]

    # ===== 3. UNIR TODAS LAS MALLAS EN UN SOLO OBJETO =====
    if len(mesh_objects) > 1:
        bpy.ops.object.join()
    joined = bpy.context.active_object
    joined.name = "Cortana_Holografica"
    print(f"Objeto unificado: {joined.name} ({len(joined.data.vertices)} vertices)")

    # ===== 4. APLICAR MODIFICADORES (Subsurf, etc.) =====
    for mod in list(joined.modifiers):
        try:
            bpy.context.view_layer.objects.active = joined
            bpy.ops.object.modifier_apply(modifier=mod.name)
            print(f"  Modificador aplicado: {mod.name}")
        except Exception as e:
            print(f"  No se pudo aplicar modificador {mod.name}: {e}")

    # ===== 5. APLICAR TRANSFORMADAS =====
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    print("Transformadas aplicadas")

    # ===== 6. ASEGURAR MATERIAL HOLOGRAFICO =====
    if "CortanaHolografico" in bpy.data.materials:
        joined.data.materials.clear()
        joined.data.materials.append(bpy.data.materials["CortanaHolografico"])
        print(f"Material aplicado: CortanaHolografico")

    # ===== 7. ESTADISTICAS =====
    tri_count = sum(len(p.vertices) - 2 for p in joined.data.polygons)
    print(f"Triangulos estimados: {tri_count}")
    print(f"Vertices: {len(joined.data.vertices)}")
    print(f"Dimensiones: {tuple(joined.dimensions)}")

    # ===== 8. EXPORTAR A GLB =====
    # Seleccionar SOLO el objeto final (no la referencia, camara, luces).
    bpy.ops.object.select_all(action="DESELECT")
    joined.select_set(True)
    bpy.context.view_layer.objects.active = joined

    output_path = (
        "/home/macarthur/Proyectos/SynapseCortana/frontend/assets/cortana_completa.glb"
    )
    bpy.ops.export_scene.gltf(
        filepath=output_path,
        export_format="GLB",
        export_apply=True,
        use_selection=True,
    )
    print(f"\n✅ Exportado a: {output_path}")
    print(f"   Objeto: {joined.name}")
    print(f"   Vertices: {len(joined.data.vertices)}")
    print(f"   Triangulos: ~{tri_count}")
    print(f"   Material: holográfico azul #1E90FF")
