-- ═══════════════════════════════════════════════════════════════════════
-- Entrega 2 · AST
-- ▶ ANALIZAR → pestaña AST (y compararla con ÁRBOL SINTÁCTICO).
-- El árbol de ANTLR tiene un nodo por regla de la gramática; el AST solo
-- lo que importa: Select, Binary, ColumnRef, InSubquery, ...
-- ═══════════════════════════════════════════════════════════════════════
DROP DATABASE IF EXISTS entrega2;
CREATE DATABASE entrega2;
USE entrega2;

CREATE TABLE estudiantes (id INT PRIMARY KEY, nombre VARCHAR(30) NOT NULL, edad INT);
CREATE TABLE inscripciones (estudiante_id INT, curso_id INT, nota FLOAT);

-- Precedencia: el AST deja ver que * va antes que + y AND antes que OR.
SELECT nombre FROM estudiantes WHERE edad + 2 * 3 > 20 OR NOT edad < 18 AND nombre <> 'Ana';

-- `-5` se pliega a un literal negativo; los identificadores sin comillas
-- quedan en minúsculas.
INSERT INTO Estudiantes (ID, Nombre, Edad) VALUES (1, 'Ana', -5);

-- Subconsultas: InSubquery, Exists (NOT EXISTS queda como Exists negated)
-- y ScalarSubquery.
SELECT nombre FROM estudiantes
WHERE id IN (SELECT estudiante_id FROM inscripciones WHERE curso_id = 3);

SELECT e.nombre FROM estudiantes e
WHERE NOT EXISTS (SELECT 1 FROM inscripciones i WHERE i.estudiante_id = e.id)
  AND e.edad > (SELECT AVG(edad) FROM estudiantes);

-- JOIN, agregados y GROUP BY: cada cláusula es una arista rotulada.
SELECT e.nombre, COUNT(*) AS n, AVG(i.nota) AS prom
FROM estudiantes e INNER JOIN inscripciones i ON i.estudiante_id = e.id
GROUP BY e.nombre HAVING COUNT(*) > 1 ORDER BY prom DESC;
