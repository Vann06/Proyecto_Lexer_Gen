-- ═══════════════════════════════════════════════════════════════════════
-- Entrega 8 · Integración: un caso completo de punta a punta
-- Un sistema de notas que usa todo lo anterior en un mismo script:
-- esquema, carga, consultas, reportes, índices, subconsultas y cambios.
-- ▶ ANALIZAR (cero problemas) y después ▶ EJECUTAR → RESULTADOS y CATÁLOGO.
-- Se puede repetir: empieza con su base limpia.
-- ═══════════════════════════════════════════════════════════════════════
DROP DATABASE IF EXISTS universidad;
CREATE DATABASE universidad;
USE universidad;

-- 1. Esquema --------------------------------------------------------------
CREATE TABLE estudiantes (
    id       INT PRIMARY KEY,
    nombre   VARCHAR(40) NOT NULL,
    carne    VARCHAR(10) UNIQUE,
    edad     INT,
    activo   BOOLEAN,
    ingreso  DATE,
    CHECK (edad >= 0)
);
CREATE TABLE cursos (id INT PRIMARY KEY, nombre VARCHAR(30) NOT NULL, creditos INT);
CREATE TABLE inscripciones (
    estudiante_id INT,
    curso_id      INT,
    nota          FLOAT,
    PRIMARY KEY (estudiante_id, curso_id),
    FOREIGN KEY (estudiante_id) REFERENCES estudiantes (id),
    FOREIGN KEY (curso_id) REFERENCES cursos (id)
);
CREATE INDEX idx_edad ON estudiantes (edad);
CREATE INDEX idx_insc_est ON inscripciones (estudiante_id);

-- 2. Carga ----------------------------------------------------------------
INSERT INTO estudiantes VALUES
    (1, 'Ana',            'A001', 20,   TRUE,  '2023-01-15'),
    (2, 'Luis',           'A002', 17,   TRUE,  '2024-01-10'),
    (3, 'Marta',          'A003', 22,   FALSE, '2022-07-01'),
    (4, 'Pedro',          'A004', 19,   TRUE,  '2023-08-20'),
    (5, 'Sofía',          NULL,   16,   TRUE,  '2025-01-08'),
    (6, 'O''Brien, José', NULL,   NULL, NULL,  '2024-02-29');
INSERT INTO cursos VALUES (1, 'Compiladores', 5), (2, 'Bases de datos', 4), (3, 'Redes', 3), (4, 'Estadística', 2);
INSERT INTO inscripciones VALUES
    (1, 1, 91), (1, 3, 55.5), (2, 3, 80), (3, 1, 70), (3, 2, 88), (4, 1, 45), (4, 3, 62), (6, 2, NULL);

-- 3. Consultas básicas (el ejemplo del enunciado usa idx_edad) -------------
SELECT nombre FROM estudiantes WHERE edad >= 18;
SELECT nombre, edad FROM estudiantes WHERE activo = TRUE AND (edad < 18 OR ingreso < '2023-06-01') ORDER BY edad DESC;

-- 4. Boleta de notas: JOIN de las tres tablas (los JOIN usan índices) -------
SELECT e.nombre, c.nombre AS curso, i.nota
FROM estudiantes e
INNER JOIN inscripciones i ON i.estudiante_id = e.id
INNER JOIN cursos c ON c.id = i.curso_id
ORDER BY e.nombre, curso;

-- 5. Reportes: agregados, GROUP BY y HAVING --------------------------------
SELECT c.nombre, COUNT(*) AS inscritos, COUNT(i.nota) AS con_nota, AVG(i.nota) AS promedio,
       MIN(i.nota) AS minima, MAX(i.nota) AS maxima
FROM cursos c
INNER JOIN inscripciones i ON i.curso_id = c.id
GROUP BY c.nombre
ORDER BY promedio DESC;

SELECT e.nombre, SUM(c.creditos) AS creditos
FROM estudiantes e
INNER JOIN inscripciones i ON i.estudiante_id = e.id
INNER JOIN cursos c ON c.id = i.curso_id
GROUP BY e.nombre
HAVING SUM(c.creditos) >= 8
ORDER BY creditos DESC, e.nombre;

-- 6. Subconsultas -----------------------------------------------------------
SELECT nombre FROM estudiantes WHERE id IN (SELECT estudiante_id FROM inscripciones WHERE curso_id = 3);
SELECT nombre FROM estudiantes WHERE id NOT IN (SELECT estudiante_id FROM inscripciones);
SELECT e.nombre FROM estudiantes e
WHERE EXISTS (SELECT 1 FROM inscripciones i WHERE i.estudiante_id = e.id AND i.nota >= 85);
SELECT c.nombre FROM cursos c WHERE NOT EXISTS (SELECT 1 FROM inscripciones i WHERE i.curso_id = c.id);
SELECT nombre, edad FROM estudiantes WHERE edad > (SELECT AVG(edad) FROM estudiantes) ORDER BY edad DESC;

-- 7. Cambios: curva de +5 a quienes reprobaron, bajas y cierre de ciclo ----
UPDATE inscripciones SET nota = nota + 5
WHERE nota < 60 AND estudiante_id IN (SELECT id FROM estudiantes WHERE activo = TRUE);
DELETE FROM inscripciones WHERE nota IS NULL;
UPDATE estudiantes SET activo = FALSE
WHERE NOT EXISTS (SELECT 1 FROM inscripciones i WHERE i.estudiante_id = estudiantes.id);

-- 8. Estado final -----------------------------------------------------------
SELECT e.nombre, e.activo, (SELECT COUNT(*) FROM inscripciones i WHERE i.estudiante_id = e.id) AS cursos,
       (SELECT AVG(nota) FROM inscripciones i WHERE i.estudiante_id = e.id) AS promedio
FROM estudiantes e
ORDER BY e.id;
SHOW TABLES;
