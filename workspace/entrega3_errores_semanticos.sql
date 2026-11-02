-- Entrega 3 · análisis semántico: un error por línea, para ▶ ANALIZAR →
-- PROBLEMAS. La sintaxis de todo el archivo es válida: cada error lo detecta
-- el análisis semántico contra el catálogo, con su línea y columna.
-- (Con ▶ EJECUTAR se detiene en el primero, la línea del SEM001.)
CREATE DATABASE IF NOT EXISTS entrega3;
USE entrega3;
DROP TABLE IF EXISTS estudiantes;
DROP TABLE IF EXISTS inscripciones;
CREATE TABLE estudiantes (id INT PRIMARY KEY, nombre VARCHAR(10) NOT NULL, edad INT, ingreso DATE);
CREATE TABLE inscripciones (estudiante_id INT, curso_id INT, nota FLOAT);
CREATE INDEX idx_edad ON estudiantes (edad);

SELECT * FROM profesores;                                                   -- SEM001 tabla inexistente
CREATE TABLE estudiantes (x INT);                                           -- SEM002 tabla ya existe
SELECT apellido FROM estudiantes;                                           -- SEM003 columna inexistente
SELECT nombre FROM estudiantes e INNER JOIN estudiantes f ON e.id = f.id;   -- SEM004 columna ambigua
SELECT x.nombre FROM estudiantes e;                                         -- SEM005 calificador inválido
SELECT nombre FROM estudiantes WHERE edad >= 'dieciocho';                   -- SEM006 tipos incompatibles
SELECT nombre FROM estudiantes WHERE ingreso > '2024-13-45';                -- SEM006 fecha inválida
INSERT INTO inscripciones VALUES (1, 2);                                    -- SEM007 cantidad de valores
INSERT INTO estudiantes VALUES (1, 'Un nombre muy largo', 20, NULL);        -- SEM008 VARCHAR(10) excedido
INSERT INTO estudiantes (id, edad) VALUES (2, 19);                          -- SEM009 nombre es NOT NULL
DROP INDEX idx_nota;                                                        -- SEM010 índice inexistente
CREATE INDEX idx_edad ON estudiantes (edad);                                -- SEM011 índice duplicado
SELECT nombre FROM estudiantes WHERE edad = (SELECT edad, id FROM estudiantes); -- SEM012 dos columnas
SELECT curso_id, nota FROM inscripciones GROUP BY curso_id;                 -- SEM013 fuera de GROUP BY
SELECT UPPER(nombre) FROM estudiantes;                                      -- SEM014 función desconocida
USE biblioteca;                                                             -- SEM015 base inexistente
CREATE DATABASE principal;                                                  -- SEM016 base ya existe
CREATE TABLE t (a INT, a INT);                                              -- SEM017 columna repetida
CREATE TABLE u (a INT PRIMARY KEY, b INT PRIMARY KEY);                      -- SEM018 dos PRIMARY KEY
SELECT * FROM estudiantes e LEFT JOIN inscripciones i ON i.estudiante_id = e.id; -- SEM099 no soportado
