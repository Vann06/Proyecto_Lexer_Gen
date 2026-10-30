-- ═══════════════════════════════════════════════════════════════════════
-- Entrega 6 · INNER JOIN, agregaciones y GROUP BY
-- ▶ EJECUTAR → RESULTADOS. Se puede repetir: empieza con su base limpia.
-- ═══════════════════════════════════════════════════════════════════════
DROP DATABASE IF EXISTS entrega6;
CREATE DATABASE entrega6;
USE entrega6;

CREATE TABLE estudiantes (id INT PRIMARY KEY, nombre VARCHAR(30) NOT NULL, edad INT);
CREATE TABLE cursos (id INT PRIMARY KEY, nombre VARCHAR(30) NOT NULL, creditos INT);
CREATE TABLE inscripciones (estudiante_id INT, curso_id INT, nota FLOAT,
                            PRIMARY KEY (estudiante_id, curso_id));
INSERT INTO estudiantes VALUES (1, 'Ana', 20), (2, 'Luis', 17), (3, 'Marta', 22), (4, 'Pedro', 19), (5, 'Sofía', 16);
INSERT INTO cursos VALUES (1, 'Compiladores', 5), (2, 'Bases de datos', 4), (3, 'Redes', 3), (4, 'Estadística', 2);
INSERT INTO inscripciones VALUES
    (1, 1, 91), (1, 3, 55.5), (2, 3, 80), (3, 1, 70), (3, 2, 88), (4, 1, 45), (4, 3, NULL);

-- INNER JOIN de dos y de tres tablas, con alias.
SELECT e.nombre, i.nota FROM estudiantes e
INNER JOIN inscripciones i ON i.estudiante_id = e.id
ORDER BY e.nombre, i.nota DESC;

SELECT e.nombre, c.nombre AS curso, i.nota
FROM estudiantes e
INNER JOIN inscripciones i ON i.estudiante_id = e.id
INNER JOIN cursos c ON c.id = i.curso_id
WHERE i.nota >= 70
ORDER BY curso, e.nombre;

-- COUNT, SUM, AVG, MIN y MAX sobre toda la tabla. COUNT(*) cuenta filas;
-- COUNT(nota) y los demás ignoran el NULL de Pedro en Redes.
SELECT COUNT(*) AS filas, COUNT(nota) AS con_nota, SUM(nota) AS suma,
       AVG(nota) AS promedio, MIN(nota) AS minima, MAX(nota) AS maxima
FROM inscripciones;

SELECT COUNT(DISTINCT curso_id) AS cursos_con_inscritos FROM inscripciones;

-- GROUP BY: estadísticas por curso (Estadística no aparece: no tiene inscritos).
SELECT c.nombre, COUNT(*) AS inscritos, AVG(i.nota) AS promedio, MAX(i.nota) AS mejor
FROM cursos c
INNER JOIN inscripciones i ON i.curso_id = c.id
GROUP BY c.nombre
ORDER BY promedio DESC;

-- HAVING: estudiantes con 8 créditos o más, ordenados por el agregado.
SELECT e.nombre, SUM(c.creditos) AS creditos, COUNT(*) AS cursos
FROM estudiantes e
INNER JOIN inscripciones i ON i.estudiante_id = e.id
INNER JOIN cursos c ON c.id = i.curso_id
GROUP BY e.nombre
HAVING SUM(c.creditos) >= 8
ORDER BY creditos DESC, e.nombre;
