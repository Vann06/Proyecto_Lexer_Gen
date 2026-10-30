-- ═══════════════════════════════════════════════════════════════════════
-- Entrega 7 · Índices y subconsultas
-- ▶ EJECUTAR → RESULTADOS: cada tarjeta muestra el plan ("índice idx_edad"
-- o "recorrido completo"). Se puede repetir: empieza con su base limpia.
-- ═══════════════════════════════════════════════════════════════════════
DROP DATABASE IF EXISTS entrega7;
CREATE DATABASE entrega7;
USE entrega7;

CREATE TABLE estudiantes (id INT PRIMARY KEY, nombre VARCHAR(30) NOT NULL, edad INT);
CREATE TABLE cursos (id INT PRIMARY KEY, nombre VARCHAR(30) NOT NULL, creditos INT);
CREATE TABLE inscripciones (estudiante_id INT, curso_id INT, nota FLOAT,
                            PRIMARY KEY (estudiante_id, curso_id));
INSERT INTO estudiantes VALUES (1, 'Ana', 20), (2, 'Luis', 17), (3, 'Marta', 22), (4, 'Pedro', 19), (5, 'Sofía', 16);
INSERT INTO cursos VALUES (1, 'Compiladores', 5), (2, 'Bases de datos', 4), (3, 'Redes', 3), (4, 'Estadística', 2);
INSERT INTO inscripciones VALUES (1, 1, 91), (1, 3, 55.5), (2, 3, 80), (3, 1, 70), (3, 2, 88), (4, 1, 45);

-- ── Índices ─────────────────────────────────────────────────────────────
-- Sin índice: recorre toda la tabla.
SELECT nombre FROM estudiantes WHERE edad >= 18;

-- CREATE INDEX guarda idx_edad.idx (valor|rowids, ordenado por valor).
CREATE INDEX idx_edad ON estudiantes (edad);

-- Con índice: igualdad, rangos y AND usan búsqueda binaria en el .idx.
SELECT nombre FROM estudiantes WHERE edad >= 18;
SELECT nombre FROM estudiantes WHERE edad = 17;
SELECT nombre FROM estudiantes WHERE edad > 16 AND edad < 20;

-- El JOIN también usa índices: cursos.id tiene el implícito de su PRIMARY KEY.
SELECT i.nota, c.nombre FROM inscripciones i INNER JOIN cursos c ON c.id = i.curso_id;

-- INSERT, UPDATE y DELETE mantienen el índice al día.
INSERT INTO estudiantes VALUES (6, 'Nuevo', 18);
UPDATE estudiantes SET edad = 18 WHERE nombre = 'Luis';
DELETE FROM estudiantes WHERE edad = 16;
SELECT nombre FROM estudiantes WHERE edad = 18 ORDER BY nombre;

-- DROP INDEX: la misma consulta vuelve al recorrido completo.
DROP INDEX idx_edad;
SELECT nombre FROM estudiantes WHERE edad = 18 ORDER BY nombre;

-- ── Subconsultas ────────────────────────────────────────────────────────
-- El ejemplo del enunciado (IN).
SELECT nombre
FROM estudiantes
WHERE id IN (SELECT estudiante_id
             FROM inscripciones
             WHERE curso_id = 3);

-- NOT IN: estudiantes sin ninguna inscripción.
SELECT nombre FROM estudiantes WHERE id NOT IN (SELECT estudiante_id FROM inscripciones);

-- EXISTS / NOT EXISTS correlacionados (usan e.id / c.id de afuera).
SELECT e.nombre FROM estudiantes e
WHERE EXISTS (SELECT 1 FROM inscripciones i WHERE i.estudiante_id = e.id AND i.nota >= 85);

SELECT c.nombre FROM cursos c
WHERE NOT EXISTS (SELECT 1 FROM inscripciones i WHERE i.curso_id = c.id);

-- Escalares en comparaciones (=, <, >).
SELECT nombre, edad FROM estudiantes WHERE edad > (SELECT AVG(edad) FROM estudiantes);
SELECT nombre FROM estudiantes WHERE edad = (SELECT MAX(edad) FROM estudiantes);
SELECT nombre FROM cursos WHERE creditos < (SELECT MAX(creditos) FROM cursos) ORDER BY nombre;

-- Escalar correlacionada en el SELECT y subconsultas anidadas.
SELECT e.nombre, (SELECT COUNT(*) FROM inscripciones i WHERE i.estudiante_id = e.id) AS cursos
FROM estudiantes e ORDER BY e.nombre;

SELECT nombre FROM estudiantes
WHERE id IN (SELECT estudiante_id FROM inscripciones
             WHERE curso_id IN (SELECT id FROM cursos WHERE creditos >= 4))
ORDER BY nombre;
