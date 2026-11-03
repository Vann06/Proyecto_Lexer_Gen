# Speech — Fase de integración (entregas 6, 7 y 8)

Duración estimada: **12–14 minutos** de exposición + 3 minutos de demo.
Cada bloque corresponde a una diapositiva. Las notas del orador del deck tienen
este mismo texto.

## Antes de presentar (5 min)

1. Levantar todo: `docker compose up --build` (IDE en :4000, API en :8080).
2. En el IDE, abrir `workspace/entrega8_integracion.sql` y ejecutarlo una vez:
   crea la base `universidad`, sus datos y el índice `idx_edad` que usa la demo.

---

## 1. Portada (30 s)

Buenas. Vamos a presentar la fase de integración de nuestro intérprete SQL.
La idea es doble: mostrar qué se implementó en esta fase y, sobre todo,
explicar cómo lo hicimos por dentro. Todo lo que van a ver corre sobre
archivos de texto propios: no usamos SQLite, MySQL ni ningún otro motor. El
lexer y el parser salen de una gramática ANTLR, el resto es Python, la API es
FastAPI y el IDE es React en el navegador.

## 2. Lo que pide el enunciado (1 min)

El enunciado es claro: interpretar y ejecutar SQL con un almacenamiento
propio. Las bases son directorios y las tablas son archivos, y está prohibido
apoyarse en un DBMS. Sobre eso pide cuatro bloques: cinco tipos de datos
guardados en el esquema; un subconjunto de SQL que incluye JOIN, agregados,
GROUP BY, HAVING, IN y EXISTS; índices en archivos que el WHERE use en vez de
recorrer la tabla y que se mantengan al día; y una interfaz que muestre cada
etapa del compilador. Esta fase cubre las entregas 6, 7 y 8.

## 3. Pipeline (1 min)

Esta es la columna vertebral, la misma que dibuja el enunciado. ANTLR genera
el lexer y el parser desde `SQL.g4`; los errores ahí son LEX o SYN. Después,
`builder.py` recorre el árbol con un visitor y produce un AST propio. La
semántica lo valida contra el catálogo y emite errores SEM. Si todo está bien,
el ejecutor interpreta el AST y lee o escribe archivos.

Un detalle importante: la semántica corre **sentencia por sentencia** contra el
catálogo ya actualizado. Si la primera línea crea una tabla, la segunda tiene
que poder usarla.

## 4. Almacenamiento (1 min)

Cada base es una carpeta. `metadata.txt` guarda el esquema en texto legible.
Cada tabla es un CSV con una columna extra, `_rowid`, un número que nunca
cambia ni se reutiliza porque los índices apuntan a él. Los índices son
archivos `.idx` con las claves ordenadas, y el `.csv.pos` guarda en qué byte
empieza cada fila. Toda escritura es atómica: se escribe un temporal y se
reemplaza de una vez.

## 5. Qué se hizo en esta fase (40 s)

La entrega 6 pide JOIN, agregaciones y GROUP BY: dos módulos nuevos,
`join.py` y `aggregate.py`. La 7 pide índices y subconsultas: extendimos los
índices al JOIN y agregamos `subquery.py`. La 8 es la integración: ejecutar
scripts completos desde la API y el IDE. Además, las FOREIGN KEY ahora se
hacen cumplir.

## 6. INNER JOIN (1 min 15 s)

Por cada JOIN decidimos cómo buscar las filas de la tabla unida. La forma
simple son bucles anidados: cada fila de la izquierda contra todas las de la
derecha. Pero si el ON tiene `tabla_unida.columna = algo conocido` y esa
columna tiene índice, buscamos la clave en el índice y leemos del CSV solo las
filas que coinciden. En ambos casos se vuelve a evaluar el ON completo, así
que el resultado es idéntico. Cada consulta devuelve un plan que dice qué
estrategia usó, y las pruebas comparan las dos con datos aleatorios.

## 7. Agregados y GROUP BY (1 min 15 s)

`aggregate.py` agrupa las filas que pasaron el WHERE por el valor del GROUP
BY. Cada grupo tiene un entorno con un callback que calcula COUNT, SUM, AVG,
MIN y MAX sobre sus filas, y guarda el resultado para no repetirlo en HAVING
u ORDER BY. Seguimos el estándar: los NULL forman un grupo, `COUNT(*)` de una
tabla vacía da cero y AVG siempre es decimal. La semántica rechaza antes los
usos inválidos, como un agregado dentro del WHERE (SEM013).

## 8. Índices (1 min 15 s)

Es el requisito de “usar el índice en lugar de recorrer todos los
registros”. Tres pasos: búsqueda binaria en el `.idx` para obtener los
rowids; el `.pos` dice en qué byte está cada fila; y un `seek` lee solo esa
fila. Cada INSERT, UPDATE o DELETE reconstruye los índices de la tabla. Para
demostrar que no se recorre la tabla, una prueba intercepta la lectura
completa del CSV y falla si alguien la llama.

## 9. Subconsultas (1 min 15 s)

El entorno es una pila de ámbitos. La subconsulta apila sus tablas encima de
la fila externa: si adentro aparece `e.id`, el evaluador baja un nivel y lo
encuentra afuera. Eso es la correlación. Si la subconsulta no nombra ninguna
columna de afuera, la ejecutamos una vez y la guardamos en caché durante la
sentencia; si es correlacionada, se ejecuta por fila. Respetamos la lógica de
tres valores: `x NOT IN (…, NULL)` nunca es verdadero.

## 10. Análisis semántico (1 min)

Cada error que lista el enunciado tiene su código y su ubicación: SEM001 tabla
inexistente, SEM003 columna, SEM006 tipos, SEM007 a 009 valores del INSERT,
SEM004 y 005 referencias, SEM010 y 011 índices. La subconsulta escalar con más
de una columna es SEM012. Cuántas **filas** devuelve depende de los datos, así
que eso se detecta al ejecutar, como EXE003, también con su ubicación exacta.

## 11. FOREIGN KEY (1 min)

Las FK se guardaban pero no se hacían cumplir; en esta fase lo cerramos. Del
lado hijo, un INSERT o UPDATE no puede dejar una llave que no exista en el
padre. Del lado padre aplicamos RESTRICT: no se puede borrar ni cambiar una
fila referenciada, ni hacer DROP de la tabla. Todo se valida en memoria antes
de escribir: si falla, ningún archivo cambia.

## 12. Integración: API e IDE (1 min)

La API tiene `parse`, que analiza sin ejecutar; `run`, que ejecuta sentencia
por sentencia y se detiene en el primer error; y `catalog`, que expone el
esquema. El IDE tiene una pestaña por cada cosa que pide el enunciado. En esta
fase sumamos EJECUTAR, RESULTADOS (con el plan) y CATÁLOGO.

## 13. Evidencia (45 s)

450 pruebas en verde, en local y en CI. `test_rubrica.py` recorre el
enunciado punto por punto a través de la API. Hay un script de demo por
entrega en `workspace/`. Y CI regenera el parser desde la gramática y falla si
no coincide con el versionado.

## 14. Decisiones y límites (45 s)

Índices de una columna, usados con igualdades, rangos y AND; con OR se recorre
la tabla. Cada mutación reescribe el CSV completo: simple y atómico. Solo
INNER JOIN, que es lo que pide el enunciado. FK con RESTRICT, sin cascada. Y
sin transacciones entre archivos, pero todo se valida antes de tocar el disco.

## 15. Demo y cierre (3 min)

1. Escribir `SELECT nombre FROM estudiantes WHERE edad >= 18;`.
2. ANALIZAR: mostrar TOKENS, ÁRBOL SINTÁCTICO y AST.
3. EJECUTAR: en RESULTADOS solo aparecen los mayores de edad y el plan dice
   `índice idx_edad`.
4. Error a propósito: `WHERE edad = 'hola'` → PROBLEMAS muestra SEM006 con su
   línea y columna.

Con eso cubrimos el recorrido completo que pide el enunciado: del SQL al
archivo de texto y de vuelta al resultado. Gracias, quedamos atentos a sus
preguntas.

---

## Preguntas probables

- **¿Por qué el error de “más de una fila” no es semántico?** Porque depende
  de los datos al momento de ejecutar; la semántica no los conoce.
- **¿Cómo prueban que se usa el índice?** El resultado trae el plan, y una
  prueba falla si se lee el CSV completo en una consulta indexada.
- **¿Qué pasa si se corta la luz a mitad de un INSERT?** Cada archivo se
  escribe en un temporal y se reemplaza; no queda un archivo cortado. No hay
  transacción entre varios archivos.
- **¿Por qué no LEFT JOIN?** No lo pide el enunciado; se rechaza con SEM099.
