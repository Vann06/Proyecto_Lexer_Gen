-- demo.sql — recorrido por el SQL que soporta el intérprete.
-- ▶ ANALIZAR muestra tokens, árbol sintáctico, AST y problemas
-- (léxicos, sintácticos y semánticos). Este script no debe dar ninguno.

CREATE DATABASE IF NOT EXISTS universidad;
USE universidad;

-- ── Tablas: los cinco tipos y las restricciones ───────────────────────
CREATE TABLE estudiantes (
    id       INT PRIMARY KEY,
    nombre   VARCHAR(50) NOT NULL,
    carne    VARCHAR(10) UNIQUE,
    edad     INT,
    promedio FLOAT,
    activo   BOOLEAN,
    ingreso  DATE,
    CHECK (edad >= 0)
);

CREATE TABLE cursos (
    id       INT PRIMARY KEY,
    nombre   VARCHAR(50) NOT NULL,
    creditos INT
);

CREATE TABLE inscripciones (
    estudiante_id INT,
    curso_id      INT,
    nota          FLOAT,
    aprobado      BOOLEAN,
    CONSTRAINT pk_insc PRIMARY KEY (estudiante_id, curso_id),
    CONSTRAINT fk_est  FOREIGN KEY (estudiante_id) REFERENCES estudiantes (id),
    FOREIGN KEY (curso_id) REFERENCES cursos
);

CREATE INDEX idx_edad ON estudiantes (edad);

-- ── INSERT / UPDATE / DELETE ──────────────────────────────────────────
INSERT INTO estudiantes VALUES
    (1, 'Ana',      'A001', 20, 88.5, TRUE,  '2023-01-15'),
    (2, 'Luis',     'A002', 17, 75.0, TRUE,  '2024-01-10'),
    (3, 'O''Brien', NULL,   22, NULL, FALSE, '2022-07-01');

INSERT INTO cursos (id, nombre, creditos) VALUES (1, 'Compiladores', 5), (2, 'Bases de datos', 4), (3, 'Redes', 3);

INSERT INTO inscripciones VALUES (1, 1, 91, TRUE), (1, 3, 55.5, FALSE), (2, 3, 80, TRUE), (3, 2, 60, TRUE);

UPDATE estudiantes SET activo = FALSE, promedio = promedio - 1 WHERE ingreso < '2023-01-01';
DELETE FROM inscripciones WHERE nota IS NULL;

-- ── SELECT, WHERE, ORDER BY (ejemplo del enunciado) ───────────────────
SELECT nombre
FROM estudiantes
WHERE edad >= 18;

SELECT nombre, edad FROM estudiantes WHERE activo = TRUE AND (edad > 18 OR promedio >= 80) ORDER BY edad DESC, nombre;

-- ── JOIN, agregados, GROUP BY, HAVING ─────────────────────────────────
SELECT c.nombre, COUNT(*) AS inscritos, AVG(i.nota) AS promedio
FROM cursos c
INNER JOIN inscripciones i ON i.curso_id = c.id
GROUP BY c.nombre
HAVING COUNT(*) >= 1
ORDER BY promedio DESC;

SELECT MIN(edad), MAX(edad), SUM(promedio) FROM estudiantes;

-- ── Subconsultas ──────────────────────────────────────────────────────
SELECT nombre
FROM estudiantes
WHERE id IN (SELECT estudiante_id
             FROM inscripciones
             WHERE curso_id = 3);

SELECT nombre FROM estudiantes WHERE id NOT IN (SELECT estudiante_id FROM inscripciones);

SELECT e.nombre FROM estudiantes e
WHERE EXISTS (SELECT * FROM inscripciones i WHERE i.estudiante_id = e.id AND i.aprobado = TRUE);

SELECT c.nombre FROM cursos c
WHERE NOT EXISTS (SELECT 1 FROM inscripciones i WHERE i.curso_id = c.id);

SELECT nombre, edad FROM estudiantes
WHERE edad > (SELECT AVG(edad) FROM estudiantes)
ORDER BY edad DESC;

-- ── Índices y catálogo ────────────────────────────────────────────────
DROP INDEX idx_edad;
SHOW TABLES;
DESCRIBE estudiantes;
