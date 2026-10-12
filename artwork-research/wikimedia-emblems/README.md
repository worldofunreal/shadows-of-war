# Auditoría de identidad visual: Naciones y bots

Estado: material de investigación; los candidatos no están conectados al juego. Cada recurso queda ligado a una identidad del catálogo o marcado como candidato de fuente.

## Bots

El catálogo contiene **1,069 registros `tribe`**, agrupados en **1,007 familias** al unir únicamente variantes numéricas finales del nombre. Los acentos se conservan; `Embera` y `Emberá` son nombres distintos. Un bot puede venir del roster del mapa, la ubicación, las etiquetas del mapa o una reserva. La reserva principal de Tribe tiene **670 IDs** (653 `tribe` y 17 registros de otros tipos) y luego hay **416 IDs** `tribe` adicionales. [`targets.csv`](targets.csv) registra las rutas por ID.

Las familias se separan así:

- **27** ya tienen al menos un retrato propio en la familia; dos también dieron una coincidencia de título. Puede reutilizarse el retrato entre variantes numéricas después de cotejar la identidad.
- **56** tienen **62 archivos** enlazados desde propiedades de imagen de Wikidata. El enlace es comprobable en el registro de Wikidata, pero no prueba que el símbolo corresponda al periodo histórico del juego.
- **135** dieron **591 resultados** de título en Commons. Son coincidencias para revisar; **133** de esas familias no tienen retrato en el catálogo. Un nombre dentro del título no confirma identidad.
- **791** no tienen retrato ni candidato de imagen en las búsquedas completadas. Hay que verificar el registro y buscar su símbolo antes de encargar arte.

La búsqueda de identidad encontró al menos un QID candidato de Wikidata para **547 familias**; **460** no tienen candidato QID en las consultas completadas. Todos los QIDs requieren cotejo con el nombre, región y época del registro antes de llamar verificada a la identidad. Ningún candidato histórico se marca como aprobado automáticamente.

## Naciones

El catálogo actual tiene **774 registros** que no son `tribe`: 236 imperios, 214 reinos, 184 países y 140 ciudades-estado. Hay **348 candidatos** con archivo asociado (324 vínculos de Wikidata y 24 candidatos encontrados por título) y **426 sin candidato hallado**.

La selección automática no equivale a filtrar por `kind`: el roster de un mapa puede nombrar cualquier ID; las rutas geográficas y de etiquetas agregan identidades por mapa. La reserva principal de Naciones tiene **229 IDs** (200 imperios, 28 reinos y `qara_khitai`, clasificado como `tribe`), seguida por **546** identidades de otros tipos no tribales. Si esas reservas se agotan, el juego puede construir nombres de Nación a partir de los 670 IDs de la reserva Tribe. [`targets.csv`](targets.csv) registra los grupos por los que cada ID puede entrar.

## Fuentes y archivos

- [`bot-identity-audit.csv`](bot-identity-audit.csv): una fila por familia; incluye retratos existentes, candidatos de Wikidata y estado de verificación. Los QIDs encontrados son candidatos para cotejar, no identidades confirmadas.
- [`bot-source-review.csv`](bot-source-review.csv): 197 vínculos de imagen (62 de Wikidata y 135 resultados representativos por título), con descripción, categorías, fuente y estado.
- [`commons-search-results.csv`](commons-search-results.csv): los 591 resultados de título sin tratarlos como emblemas.
- [`selected-candidates.csv`](selected-candidates.csv): 348 candidatos de Nación y 62 archivos de bot enlazados desde Wikidata.
- [`commons-review-needed.csv`](commons-review-needed.csv): 24 candidatos de Nación por título y 135 resultados representativos de bot.
- [`commission-needed.csv`](commission-needed.csv): 426 Naciones y 791 familias de bot sin candidato visual encontrado; eso no prueba que no exista una fuente histórica.
- [`historical-review-needed.csv`](historical-review-needed.csv): los 362 vínculos de imagen que requieren cotejo con la identidad y su periodo.

Las imágenes de Nación conservan sus originales locales en `originals/` y las vistas cuadradas en `square-256/`. Para bots se descargaron los 197 thumbnails oficiales de Commons en `bot-candidate-raster/` y se prepararon WebP de 256×256 en `bot-candidate-square-256/`. El enlace al archivo fuente, autor, licencia y restricciones queda en la ficha; el thumbnail local no se etiqueta como original.

Rust usa archivos WebP individuales en la ruta existente de retratos, así que no hace falta una hoja de sprites para revisar estos candidatos. Las vistas mantienen la proporción dentro del cuadro cuadrado: las banderas horizontales no se estiran y los escudos no se recortan. Las vistas siguen en la carpeta de investigación, sin conexión al juego.

## Licencias

[`attribution.csv`](attribution.csv) conserva crédito sugerido, autor, licencia, URL de licencia, ficha de Commons y vista preparada por uso. [`download-manifest.csv`](download-manifest.csv) registra descarga, dimensiones y SHA-256. [`legal-review-needed.csv`](legal-review-needed.csv) separa las alertas publicadas por Commons. Preparar el crédito no aprueba el uso del archivo o del símbolo.
