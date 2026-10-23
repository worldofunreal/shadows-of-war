# Identidades para avatares de bots

Cada fila es un avatar por encargar: un retrato cuadrado de frente, una sola imagen por identidad.
La columna "Material de arte" dice qué imágenes ya existen en tu compu (en `Downloads` o en lo que generó Codex) y pueden servir de punto de partida.

## `naciones-200.csv`: los 220 bots de naciones

- La lista de Codex que aprobaste (200), más 20 de reserva al final (filas 201 a 220).
- Regiones: Asia 60, Américas 59, África 44, Europa 41, Oceanía 16. Género: 111 hombres y 109 mujeres.
- **Los 220 tienen página en Wikipedia.** El enlace está en "Referencia". Las hermanas Trưng comparten la misma página.
- Yaa Asantewaa, Lozen, Amina de Zazzau, Makeda, Anacaona, Sālote Tupou III y Hongi Hika pasaron a héroes jugables y salieron de la lista de naciones (213 bots). Su arte de héroe está en `assets/art/avatars/heroes/`.
- Se cambiaron 7 que ya son héroes jugables, con el mismo género y región. La columna "Nota" dice a quién reemplaza cada uno. Mama Ocllo entró en lugar de Mama Anahuarque, que no tiene página.
- Se cambiaron 8 que ya tenían arte de héroe (William the Conqueror, Ashoka, Darius I, Tipu Sultan, Kanishka, Rani Durgavati, John III Sobieski, Charles V). La columna "Nota" dice por quién se reemplazó cada uno.
- La columna "Nota" también marca 12 figuras del siglo XX, por si prefieres quitarlas.
- **Material de arte:** 86 de los 220 tienen imágenes registradas. No cuenta la pasada de Codex que rechazaste por la mueca repetida.
- **Avatar:** solo 5 están validados (Hatshepsut, Mansa Musa, Amanirenas, Sundiata Keita y Amanitore). Las filas marcadas "Por validar" usan imágenes del material sin asignar; varias son escenas amplias con el personaje pequeño.
- La lista con la ubicación de cada retrato está en [naciones-encargo.txt](naciones-encargo.txt): hoy **FALTAN** (0) y **TENEMOS** (213). Los retratos viven en `assets/art/avatars/nations/`.

## Avatares de tribus

La lista para encargar y revisar el arte es [tribus-400-encargo.txt](tribus-400-encargo.txt): **FALTAN** primero y **TENEMOS** después. Cada línea incluye el nombre exacto del archivo.

Incluye las tribus de bots y las tribus del tutorial que no son romanas. Ragnar, Genghis Khan, Vercingetorix y Boudica ya son héroes jugables, así que su arte es de héroe y no aparecen en esta lista. Los personajes de una misma tribu conservan su afiliación; los asentamientos no se presentan como tribus nuevas. Las seis imágenes romanas del tutorial también están en **FALTAN**: son arte viejo y se generan de nuevo como el resto.

**TENEMOS** cuenta las identidades con imagen recibida en `Downloads/extreme-closeups-avatars/`, sea JPG o PNG. **FALTAN** cuenta únicamente las identidades sin imagen en esa carpeta. Las cantidades y la carpeta de los WebP viven únicamente en el TXT.

`tribus-400.csv` conserva la investigación y los datos de las 400 identidades originales; para producir el arte se usa el TXT.

## `material-sin-asignar.csv`: arte que ya tienes y no está en ninguna lista

Está agrupado por el nombre del archivo. Algunos ejemplos son Artemisia, Baibars, Eréndira, Gajah Mada, Lobengula y Dahteste. Sirve para cambiar alguna identidad por una que ya tenga arte. También aparecen archivos que no son personajes (edificios, aldeas); esos se pueden ignorar.
