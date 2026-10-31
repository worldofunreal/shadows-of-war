# Identidades para avatares de bots

Cada fila es un avatar por encargar: un retrato cuadrado de frente, una sola imagen por identidad.
La columna "Material de arte" dice qué imágenes ya existen en tu compu (en `Downloads` o en lo que generó Codex) y pueden servir de punto de partida.

## `naciones-200.csv`: las naciones candidatas (232 filas)

- Las filas 1 a 200 son la lista de Codex que aprobaste. Las filas 201 a 232 son reemplazos, salvo la 205 (Makeda): ya es héroe jugable y no sale como nación. La columna "Nota" dice a quién reemplaza cada uno.
- Regiones: Asia 64, Américas 60, África 46, Europa 45, Oceanía 17. Género: 118 hombres y 114 mujeres.
- **Las 232 filas tienen página en Wikipedia.** El enlace está en "Referencia". Las hermanas Trưng comparten la misma página.
- Yaa Asantewaa, Lozen, Amina de Zazzau, Makeda, Anacaona, Sālote Tupou III, Hongi Hika y los 27 de la lista nueva son héroes jugables y ya no son bots de nación. Su arte de héroe está en `assets/art/avatars/heroes/`.
- Hoy hay 204 bots de nación en el juego. Se quitaron los 27 bots que tienen el nombre de un héroe; cada fila de reemplazo es otra nación de la misma región y género, y su columna "Nota" dice a quién reemplaza. Los 18 reemplazos ya tienen retrato en `assets/art/avatars/nations/`. Los 12 retratos de los bots que se quitaron en el cambio anterior (Mansa Musa, Shaka Zulu y los demás) siguen ahí sin uso.
- Se cambiaron 7 que ya son héroes jugables, con el mismo género y región. La columna "Nota" dice a quién reemplaza cada uno. Mama Ocllo entró en lugar de Lady Six Sky.
- La columna "Nota" también marca 12 figuras del siglo XX, por si prefieres quitarlas.
- **Material de arte:** 71 de los 232 tienen archivos de arte registrados en `Downloads`. No cuenta la pasada de Codex que rechazaste por la mueca repetida.
- **Avatar:** solo 4 están validados (Hatshepsut, Amanirenas, Sundiata Keita y Amanitore). Las filas marcadas "Por validar" usan imágenes del material sin asignar o retratos ya en el repositorio; varias son escenas amplias con el personaje pequeño. Los 18 retratos nuevos también figuran como "Por validar" hasta revisarlos en partida. Ya no falta ningún retrato de nación.
- La lista con la ubicación de cada retrato está en [naciones-encargo.txt](naciones-encargo.txt): hoy **FALTAN** (0) y **TENEMOS** (213). Los retratos viven en `assets/art/avatars/nations/`.

## Avatares de tribus

La lista para encargar y revisar el arte es [tribus-400-encargo.txt](tribus-400-encargo.txt): **FALTAN** primero y **TENEMOS** después. Cada línea incluye el nombre exacto del archivo.

Incluye las tribus de bots y las tribus del tutorial que no son romanas. Ragnar, Genghis Khan, Vercingetorix y Boudica ya son héroes jugables, así que su arte es de héroe y no aparecen en esta lista. Los personajes de una misma tribu conservan su afiliación; los asentamientos no se presentan como tribus nuevas. Las imágenes romanas del tutorial no van aquí: están en [tutorial-boudica-encargo.txt](tutorial-boudica-encargo.txt).

**TENEMOS** cuenta las identidades con imagen recibida en `Downloads/extreme-closeups-avatars/`, sea JPG o PNG. **FALTAN** cuenta únicamente las identidades sin imagen en esa carpeta. Las cantidades y la carpeta de los WebP viven únicamente en el TXT.

`tribus-400.csv` conserva la investigación y los datos de las 400 identidades originales; para producir el arte se usa el TXT.

## `material-sin-asignar.csv`: arte que ya tienes y no está en ninguna lista

Está agrupado por el nombre del archivo. Algunos ejemplos son Artemisia, Baibars, Eréndira, Gajah Mada, Lobengula y Dahteste. Sirve para cambiar alguna identidad por una que ya tenga arte. También aparecen archivos que no son personajes (edificios, aldeas); esos se pueden ignorar.
